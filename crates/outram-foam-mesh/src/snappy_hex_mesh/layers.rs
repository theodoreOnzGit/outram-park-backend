// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Layer addition — Phase 3 of `snappyHexMesh` (**implemented, restricted
//! scope: medial-axis interior shrink-and-insert for isolated flat/convex
//! walls, with an always-watertight outward-extrusion fallback elsewhere**).
//!
//! The final phase inserts graded prismatic ("boundary layer") cells at a wall
//! boundary patch so that near-wall gradients can be resolved. Two placements
//! are available and the driver ([`add_layers`]) picks per candidate: the
//! **interior shrink-and-insert** (the real `snappyLayerDriver` behaviour,
//! preferred) is used wherever it stays watertight, and an **outward
//! extrusion** (which never moves an existing point) is the robust fallback for
//! near-wall regions where the interior insert cannot — see "Honest scope".
//! This module ports the *geometric core* of OpenFOAM's `snappyLayerDriver`
//! (`src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C`) and its
//! medial-axis mesh mover: the per-point normal displacement field, the
//! **inward shrink of the near-wall mesh** limited by a medial-axis distance
//! (`medialAxisMeshMover::calculateDisplacement`,
//! `externalDisplacementMeshMover/medialAxisMeshMover.C:1622`; the
//! thickness/medial-distance truncation at `medialAxisMeshMover.C:1770` and
//! `:1801`, and `snappyLayerDriver::shrinkMeshMedialDistance`,
//! `snappyHexMeshDriver/snappyLayerDriverShrink.C:1348`), the graded thickness
//! distribution (`layerParameters::layerThickness`, `layerParameters.C:697`),
//! the prism-block insertion (in OpenFOAM `addPatchCellLayer::setRefinement`,
//! `polyTopoChange/polyTopoChange/addPatchCellLayer.C`), and the
//! quality-limited collapse (in OpenFOAM the `EXTRUDE`/`NOEXTRUDE` unextrusion
//! loop, `snappyLayerDriver.C:217`).
//!
//! ## Algorithm implemented here — interior shrink-and-insert
//!
//! Unlike a bare outward extrusion (which would push the domain boundary out
//! along the wall normal), this **keeps the outer wall boundary fixed** and
//! makes room for the layers by displacing the near-wall mesh *inward* on the
//! fluid side, then inserts the graded prisms into the opened gap. The domain
//! occupies exactly the same outer envelope before and after; the near-wall
//! cells are compressed rather than the boundary being extended.
//!
//! 1. **Pick the wall patch.** The first [`PatchKind::Wall`] patch (typically
//!    the carved surface produced by castellation) is chosen as the layer
//!    patch; every other patch is untouched.
//! 2. **Point normals + medial-axis distance.** Each wall *point* gets a normal
//!    equal to the area-weighted average of the OUTWARD area vectors of the
//!    incident wall faces, normalised (this is the shrink/insert direction).
//!    Each point also gets a **medial-axis distance proxy**: the smallest
//!    incident owner-cell depth normal to the wall, `owner cell volume /
//!    wall-face area` — a first-order stand-in for `medialDist_` in
//!    `medialAxisMeshMover.C:1766` (the distance from the wall to where
//!    displacements from opposite walls would meet).
//! 3. **Graded, capped block thickness.** For `n = n_surface_layers` layers
//!    with first-layer thickness `t` and expansion ratio `r` the thicknesses
//!    are `[t, t·r, t·r², …]` ([`layer_thicknesses`]); their sum `T` is the
//!    block thickness. `T` is scaled down per point by whichever of two caps
//!    binds: the face-size cap ([`LayerControls::max_thickness_fraction`] `· √A`,
//!    the convex-corner self-intersection guard) or the **medial-axis cap**
//!    ([`LayerControls::medial_axis_thickness_ratio`] `· owner-depth`, which
//!    keeps the inward shrink from inverting the owner cell — the analogue of
//!    `maxThicknessToMedialRatio`, `medialAxisMeshMover.C:1646`).
//! 4. **Shrink + graded rings.** The original wall points are moved INWARD by
//!    the capped block thickness `T_l` (shrinking the owner cells and carrying
//!    the old wall faces to the new fluid-side interface). `n` new point rings
//!    are laid from that shrunk interface back out to the ORIGINAL wall
//!    position, so ring `n` coincides with the untouched boundary. The grading
//!    is **reversed** relative to a bare extrusion: the THINNEST layer (`t`)
//!    sits adjacent to the wall and the thickest adjacent to the owner cell —
//!    the correct near-wall distribution.
//! 5. **Prism cells.** For each wall face (an `m`-gon) `n` stacked prism cells
//!    are created between successive point rings. The *original* wall face
//!    (now at the shrunk interface) becomes an INTERNAL face between its old
//!    owner cell and the first prism; interfaces between successive prisms are
//!    internal; side faces on edges shared by two wall faces are internal
//!    (owner→neighbour wound); side faces on the patch rim tile the gap the
//!    shrink opened on the domain's side boundaries; the outer cap of the
//!    outermost prism lands on the original wall and forms the NEW wall patch
//!    (same name/kind). Rim sides form a `layerSide` wall patch. Everything is
//!    assembled into a fresh [`PolyPatchMesh`] (internal faces first, then
//!    boundary patches) and rebuilt with [`PolyPatchMesh::build_fvmesh`].
//! 6. **Watertight + quality gate, with mode and layer-count fallback.** Each
//!    candidate is checked for zero non-positive-volume cells (min volume `≥`
//!    [`QualityLimits::min_vol`]) AND watertightness (every cell's signed
//!    face-area-vector sum vanishes to a tiny fraction of the largest face
//!    area). For each layer count — the full count first, then fewer — the
//!    interior insert is tried first; if it fails the gate the **outward
//!    extrusion** of the same count is tried (it moves no existing point, so it
//!    is always watertight); if neither passes, one fewer layer is tried, down
//!    to zero (original mesh returned unchanged). **No mesh with a
//!    negative-volume or non-watertight cell is ever returned.** The
//!    medial-axis + face-size caps in step 3 normally prevent inversion
//!    outright, so the volume half of the gate is a coarse backstop; the
//!    watertight half is what routes hanging-node / embedded near-wall regions
//!    to the outward-extrusion fallback (see "Honest scope").
//!
//! ## Honest scope — what IS and is NOT modelled
//!
//! **What is now real (vs the earlier extrude-outward primitive).** The layers
//! are inserted *inside* the original domain: the outer wall boundary does not
//! move, the near-wall owner cells are compressed by an inward point
//! displacement, and the prism block fills the opened gap with the thinnest
//! layer at the wall. The inward displacement is limited by a medial-axis-style
//! distance cap so the owner cell cannot invert. This is the
//! shrink-then-`addPatchCellLayer` structure of the real driver, reduced to its
//! geometric essentials and verified on a flat wall.
//!
//! **Where the interior insert applies, and the fallback.** Moving the wall
//! points is only watertight when no wall point is a **hanging node** on a
//! coarser neighbour: on an octree-refined castellated mesh a wall point that
//! sits at the mid-edge of a coarse cell's face is not tracked by that coarse
//! face, so displacing it opens a coarse/fine gap and the coarse cell fails
//! closure. The driver therefore restricts the interior insert to regions where
//! it stays watertight — in practice **uniform / isolated flat or convex
//! near-wall regions** (the hand-built flat-wall V&V case) — and falls back to
//! the earlier **outward-extrusion primitive** everywhere it does not (a carved,
//! refined surface patch such as the sphere-in-box case). The fallback grows the
//! prism block *outward* along the wall normal and so extends the mesh rather
//! than inserting inside it; it is a correct, watertight, quality-checked
//! extrusion but NOT the interior-conserving behaviour. Turning the whole
//! carved-surface case into a true interior insert needs the point-displacement
//! machinery below.
//!
//! **What still differs from OpenFOAM's `snappyLayerDriver`.** Several parts of
//! the full medial-axis mover are deliberately NOT ported and are honest
//! limitations of this increment:
//! - **Single-cell shrink, no interior smoothing.** Only the wall points are
//!   displaced, so the whole block thickness is absorbed by the *one* near-wall
//!   cell. OpenFOAM smooths the displacement several cells deep
//!   (`nSmoothDisplacement`, lambda-mu smoothing, `smoothLambdaMuDisplacement`,
//!   `medialAxisMeshMover.C:1424`) so the compression spreads into the interior.
//!   This restricts the safe block thickness to a fraction of the *first* cell's
//!   depth, not the medial distance of the whole channel.
//! - **Medial distance is a first-order proxy** (owner-cell depth), not the true
//!   medial-axis skeleton distance computed by the point-wave in
//!   `medialAxisSmoothingInfo` (`snappyLayerDriverShrink.C:861`). It is correct
//!   for an isolated flat/convex wall but conservative in a narrow channel and
//!   not validated for concave corners where opposite walls interact.
//! - **No per-face layer termination / feature-edge handling.** Layer count is
//!   reduced globally (fewer layers everywhere), never per-point or per-face,
//!   and there is no `handleFeatureAngleLayerTerminations`
//!   (`snappyLayerDriverShrink.C:491`) sharp-edge stop or
//!   `findIsolatedRegions` island removal.
//! - **No multi-patch layer coupling** across shared feature edges, and no
//!   baffle / faceZone handling.
//!
//! ## Verification & validation (V&V)
//!
//! **Methodology.** A hand-built two-cell flat-wall [`CastellatedMesh`] (two
//! unit cubes side by side in `x`, their `z = 1` top faces forming a 2-quad
//! wall patch, fluid on the `z < 1` side) has layers inserted with `n = 3`,
//! `t = 0.1 m`, `r = 1.5`. The reference is the closed-form geometric grading:
//! thicknesses `[0.1, 0.15, 0.225] m`, total `0.475 m`. Pass criteria: the
//! outer wall boundary does NOT move outward (`max z ≤ 1` — the distinguishing
//! test vs the old extrude-outward behaviour); the owner cells shrink to
//! `z ∈ [0, 0.525]`; exactly `n` prism cells per wall face; the thinnest layer
//! (`0.1 m`) is adjacent to the wall and the successive layer heights reproduce
//! the expansion ratio; the rebuilt [`FvMesh`] validates; zero negative-volume
//! cells; each cell's signed face-area-vector sum vanishes (watertight); the
//! default quality gate accepts.
//!
//! **Results (measured 2026-07-20).** New cell count `8` (was `2`) — `3` new
//! cells per wall face. Maximum point `z = 1.000 m` (outer boundary fixed; the
//! old extrusion would have reached `z = 1.475 m`). Shrunk owner top at
//! `z = 0.525 m` (`= 1 − 0.475`). Column point heights from the wall inward
//! `[0.100, 0.150, 0.225] m` (thinnest `0.100 m` layer at the wall), successive
//! ratios `1.5000` and `1.5000` (err `< 1e-12`). Rebuilt mesh validates;
//! `n_negative_volume_cells = 0`; `min_cell_volume = 0.100 m³` (thinnest prism,
//! `1 m² × 0.1 m`); `max_non_ortho ≈ 0°`, `max_skewness ≈ 0`; default
//! [`QualityLimits`] accepts. Per-cell area-vector sum magnitude `< 1e-12 m²`
//! (watertight). An oversized `t = 100 m` request is capped by the medial-axis
//! and face-size caps to a non-inverting block (`min_cell_volume > 0`).
//!
//! A second case exercises the fallback boundary on a REFINED carved mesh (a
//! unit sphere carved from a `[-2,2]³` box at level 1, `848` cells). There the
//! interior-insert candidate is measurably NON-watertight (`max |ΣSf| ≈ 2.0e-3
//! m²`, `48` unclosed coarse cells) because wall points on hanging nodes are
//! displaced; the driver rejects it and returns the outward-extrusion fallback
//! (`1472` cells, `max |ΣSf| < 1e-9 m²` watertight, zero negative-volume cells).
//! See the tests in this module.
//!
//! [`PatchKind::Wall`]: outram_foam_basic_lib::mesh::PatchKind::Wall
//! [`FvMesh`]: outram_foam_basic_lib::mesh::FvMesh

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::{CastellatedMesh, SurfaceFace};
use crate::snappy_hex_mesh::poly_topology::{face_area_and_centre, PolyPatchMesh};
use crate::snappy_hex_mesh::QualityLimits;
use crate::MeshError;

/// Numerical tolerance for treating the expansion ratio as `1` (uniform layers).
const RATIO_UNITY_TOL: f64 = 1e-12;

/// Controls for the layer-addition phase (subset of `addLayersControls`).
///
/// The thickness of the wall-nearest layer can be given directly
/// ([`first_layer_thickness`](Self::first_layer_thickness)) or implied by a
/// target [`final_layer_thickness`](Self::final_layer_thickness); see
/// [`LayerControls::first_thickness`].
#[derive(Debug, Clone)]
pub struct LayerControls {
    /// Number of prism layers to add at the wall.
    pub n_surface_layers: usize,
    /// Geometric expansion ratio `r` between successive layers (`> 0`, usually
    /// `> 1` so cells grow away from the wall). Dimensionless.
    pub expansion_ratio: f64,
    /// Thickness of the layer nearest the wall [m]. Used directly unless
    /// [`final_layer_thickness`](Self::final_layer_thickness) is `Some`.
    pub first_layer_thickness: f64,
    /// Optional target thickness of the OUTERMOST layer [m]. When `Some`, the
    /// first-layer thickness is derived from it via the OpenFOAM
    /// `FIRST_AND_EXPANSION`/`FINAL_AND_EXPANSION` relation (see
    /// `layerParameters.C:927`): `first = final / rⁿ⁻¹` for `r ≠ 1`, so the
    /// geometric series ends on the requested final thickness.
    pub final_layer_thickness: Option<f64>,
    /// Cap on the total layer-block thickness at each wall point, as a fraction
    /// of the local wall-face size `√(face area)` [dimensionless, `(0, 1]`].
    /// If the graded total would exceed `max_thickness_fraction · √A` at a
    /// point, that point's offsets are scaled down. This is the geometric guard
    /// that keeps a prism from inverting where surface normals diverge; `0.5`
    /// keeps the block below half the local cell size.
    pub max_thickness_fraction: f64,
    /// Cap on the inward shrink at each wall point, as a fraction of the local
    /// **medial-axis distance** proxy `owner cell volume / wall-face area` (the
    /// near-wall cell's depth normal to the wall) [dimensionless, `(0, 1)`].
    /// The layer block is inserted by displacing the wall points inward by the
    /// block thickness; limiting that displacement to `medial_axis_thickness_ratio
    /// · owner-depth` keeps the shrink from collapsing/inverting the owner cell.
    /// This is the analogue of OpenFOAM's `maxThicknessToMedialRatio`
    /// (`medialAxisMeshMover.C:1646`), with the true medial-axis distance
    /// replaced by the first-order owner-cell-depth proxy (see the module docs'
    /// "Honest scope"). `0.5` keeps the owner cell at least half its original
    /// depth.
    pub medial_axis_thickness_ratio: f64,
    /// Quality thresholds the layered mesh is gated on. A candidate that yields
    /// any non-positive-volume cell (or a cell below `min_vol`) is retried with
    /// fewer layers; see the module docs.
    pub quality_limits: QualityLimits,
}

impl Default for LayerControls {
    fn default() -> Self {
        Self {
            n_surface_layers: 3,
            expansion_ratio: 1.2,
            first_layer_thickness: 1e-3,
            final_layer_thickness: None,
            max_thickness_fraction: 0.5,
            medial_axis_thickness_ratio: 0.5,
            quality_limits: QualityLimits::default(),
        }
    }
}

impl LayerControls {
    /// Effective first-layer thickness [m] — the thickness of the wall-nearest
    /// layer actually used for grading.
    ///
    /// Equals [`first_layer_thickness`](Self::first_layer_thickness) unless
    /// [`final_layer_thickness`](Self::final_layer_thickness) is `Some`, in
    /// which case it is back-solved from the requested final thickness `t_f`
    /// and expansion ratio `r` over `n` layers as `t_f / rⁿ⁻¹` (`t_f` itself
    /// when `r ≈ 1` or `n ≤ 1`). Mirrors `layerParameters::firstLayerThickness`
    /// (`layerParameters.C:927`, `FINAL_AND_EXPANSION`).
    pub fn first_thickness(&self) -> f64 {
        match self.final_layer_thickness {
            None => self.first_layer_thickness,
            Some(final_t) => {
                let n = self.n_surface_layers;
                if n <= 1 || (self.expansion_ratio - 1.0).abs() < RATIO_UNITY_TOL {
                    final_t
                } else {
                    final_t / self.expansion_ratio.powi((n - 1) as i32)
                }
            }
        }
    }
}

/// Geometric layer thicknesses `[t, t·r, t·r², …]` [m] for `n` layers with
/// effective first-layer thickness `t` ([`LayerControls::first_thickness`]) and
/// expansion ratio `r`.
///
/// This is the grading arithmetic of Phase 3 in isolation (fully testable). The
/// returned vector has length `controls.n_surface_layers`; the total boundary-
/// layer thickness is its sum, `t·(rⁿ − 1)/(r − 1)` for `r ≠ 1`. Mirrors
/// `layerParameters::layerThickness` (`layerParameters.C:697`).
pub fn layer_thicknesses(controls: &LayerControls) -> Vec<f64> {
    let mut out = Vec::with_capacity(controls.n_surface_layers);
    let mut t = controls.first_thickness();
    for _ in 0..controls.n_surface_layers {
        out.push(t);
        t *= controls.expansion_ratio;
    }
    out
}

/// Total boundary-layer thickness [m] — the sum of [`layer_thicknesses`].
pub fn total_layer_thickness(controls: &LayerControls) -> f64 {
    layer_thicknesses(controls).iter().sum()
}

/// How a candidate layer block is placed relative to the wall boundary.
///
/// The two builders share the *same* prism connectivity; they differ only in
/// where the new point rings sit (see [`build_layer_topology`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerMode {
    /// **Medial-axis interior shrink-and-insert** (preferred). The near-wall
    /// mesh is displaced *inward* on the fluid side and the graded prisms fill
    /// the opened gap; the outer wall boundary does not move. This is the real
    /// `snappyLayerDriver` behaviour but, in this reduced port, only stays
    /// watertight where no wall point is a hanging node on a coarser neighbour
    /// (uniform / isolated flat/convex near-wall regions). See the module docs.
    InteriorInsert,
    /// **Outward extrusion** (robust fallback). The graded prisms are grown
    /// *outward* along the wall normal; the original mesh is untouched and the
    /// new wall patch caps the far end. Always watertight because no existing
    /// point moves — used where interior insertion cannot keep the mesh
    /// watertight (refined / hanging-node / embedded near-wall regions).
    ExtrudeOutward,
}

/// Insert graded prism layers at the wall patch, preferring medial-axis
/// interior shrink-and-insert and falling back to outward extrusion where the
/// interior insertion cannot stay watertight (see the module docs for the full
/// algorithm, restricted scope, and V&V).
///
/// For each candidate layer count (full count first, then fewer — the
/// quality-limited collapse), the [`InteriorInsert`](LayerMode::InteriorInsert)
/// topology is tried first at the first
/// [`PatchKind::Wall`](outram_foam_basic_lib::mesh::PatchKind::Wall) patch: it
/// displaces the near-wall mesh inward and inserts `n` graded prisms into the
/// opened gap, keeping the outer boundary fixed. That candidate is accepted
/// only if it is both **watertight** (every cell's signed face-area-vector sum
/// vanishes) and free of non-positive-volume cells; otherwise the
/// [`ExtrudeOutward`](LayerMode::ExtrudeOutward) topology (always watertight) is
/// tried at the same `n`. On success the returned [`CastellatedMesh`] has:
/// - `fv_mesh` / `topology` — the rebuilt, validated layered mesh. For an
///   interior insert the original wall faces are internal at the shrunk
///   fluid-side interface and a new wall patch of the same name caps the block
///   back at the original boundary; for an outward extrusion the original wall
///   faces are internal and the new wall patch caps the far (extruded) end. Both
///   add a `layerSide` patch for rim faces.
/// - `surface_faces` — the NEW wall patch's quadrilateral faces (owner prism
///   cell + corner points); non-quad wall faces are omitted from this list,
/// - `cells_by_level` — the original counts with the new prism cells added to
///   the finest-level bucket,
/// - `points` / `max_level` — the new point list / unchanged finest level.
///
/// If no layer count in either mode meets the gate, the input mesh is returned
/// unchanged (never a mesh with a negative-volume or non-watertight cell).
///
/// # Errors
/// [`MeshError::Construction`] if the mesh has no wall patch, if the chosen wall
/// patch has no faces, or if a rebuilt mesh fails
/// [`FvMesh::validate`](outram_foam_basic_lib::mesh::FvMesh::validate).
pub fn add_layers(
    mesh: &CastellatedMesh,
    controls: &LayerControls,
) -> Result<CastellatedMesh, MeshError> {
    let wall_patch = find_wall_patch(&mesh.topology).ok_or_else(|| {
        MeshError::Construction(
            "layer addition: mesh has no PatchKind::Wall patch to extrude".to_string(),
        )
    })?;

    if mesh.topology.patches[wall_patch].size == 0 {
        return Err(MeshError::Construction(
            "layer addition: chosen wall patch has no faces".to_string(),
        ));
    }

    // Zero layers requested → identity (still a valid, negative-volume-free mesh).
    if controls.n_surface_layers == 0 {
        return Ok(mesh.clone());
    }

    // Quality-limited collapse: try the full layer count, then fall back. At each
    // count, prefer the interior shrink-and-insert; if it is not watertight
    // (hanging-node / embedded near-wall region), use the outward-extrude
    // primitive, which never moves an existing point and is always watertight.
    for n in (1..=controls.n_surface_layers).rev() {
        for mode in [LayerMode::InteriorInsert, LayerMode::ExtrudeOutward] {
            let topo = build_layer_topology(mesh, wall_patch, n, controls, mode);
            if !layered_topology_acceptable(&topo, controls) {
                continue;
            }
            let fv_mesh = topo.build_fvmesh()?;
            let surface_faces = new_wall_surface_faces(&topo, wall_patch, mesh);
            let mut cells_by_level = mesh.cells_by_level.clone();
            let added = topo.n_cells - mesh.topology.n_cells;
            if let Some(slot) = cells_by_level.get_mut(mesh.max_level) {
                *slot += added;
            }
            return Ok(CastellatedMesh {
                fv_mesh,
                points: topo.points.clone(),
                topology: topo,
                surface_faces,
                cells_by_level,
                max_level: mesh.max_level,
            });
        }
    }

    // Even one layer inverts / cannot stay watertight — return the input unchanged.
    Ok(mesh.clone())
}

/// Whether a candidate layered topology may be returned: it must have zero
/// non-positive-volume cells, a smallest cell volume at or above the quality
/// gate, AND be watertight (every cell's signed face-area-vector sum vanishes to
/// a tiny fraction of the largest face area).
///
/// The watertightness check is what rejects an interior insert whose wall points
/// sit on hanging nodes: moving such a point opens a coarse/fine gap the coarse
/// neighbour cannot follow, and the resulting cell fails closure. (Non-
/// orthogonality/skewness of the *base* mesh are outside this phase's control,
/// so the volume+closure subset is gated here; the tests additionally assert the
/// full quality gate accepts on a clean flat wall.)
fn layered_topology_acceptable(topo: &PolyPatchMesh, controls: &LayerControls) -> bool {
    let q = topo.quality();
    if q.n_negative_volume_cells != 0 || q.min_cell_volume < controls.quality_limits.min_vol {
        return false;
    }
    max_cell_closure(topo) <= 1e-9 * (max_face_area(topo) + 1e-300)
}

/// Largest face-area magnitude [m²] over all faces — the length scale the
/// watertightness tolerance in [`layered_topology_acceptable`] is measured
/// against.
fn max_face_area(topo: &PolyPatchMesh) -> f64 {
    let (areas, _c) = topo.face_geometry();
    areas.iter().map(|a| a.mag()).fold(0.0f64, f64::max)
}

/// Worst per-cell closure residual `|Σ Sf|` [m²]: for every cell, the signed sum
/// of its face area vectors (owner faces `+`, neighbour side `−`), maxed over
/// cells. Zero for a watertight mesh; a non-zero value flags an open cell.
fn max_cell_closure(topo: &PolyPatchMesh) -> f64 {
    let (areas, _c) = topo.face_geometry();
    let mut sums = vec![Vector3::ZERO; topo.n_cells];
    for f in 0..topo.n_internal_faces {
        sums[topo.owner[f]] += areas[f];
        sums[topo.neighbour[f]] += -areas[f];
    }
    for f in topo.n_internal_faces..topo.faces.len() {
        sums[topo.owner[f]] += areas[f];
    }
    sums.iter().map(|v| v.mag()).fold(0.0f64, f64::max)
}

/// Index of the first [`PatchKind::Wall`] patch, if any.
fn find_wall_patch(topo: &PolyPatchMesh) -> Option<usize> {
    topo.patches
        .iter()
        .position(|p| matches!(p.kind, PatchKind::Wall))
}

/// Global cell id of the prism cell in layer `k` (`1..=n`) above wall face `j`.
#[inline]
fn prism_cell(nc: usize, n: usize, j: usize, k: usize) -> usize {
    nc + j * n + (k - 1)
}

/// Global point id of the ring-`k` (`0..=n`) copy of wall point local index `l`.
/// Ring 0 is the original wall point; rings `1..=n` are the appended offsets.
#[inline]
fn ring_pt(np: usize, n: usize, wall_pts: &[usize], l: usize, k: usize) -> usize {
    if k == 0 {
        wall_pts[l]
    } else {
        np + l * n + (k - 1)
    }
}

/// Build the layered [`PolyPatchMesh`] for a given layer count `n` and placement
/// [`mode`](LayerMode).
///
/// The prism connectivity (the faces/owner/neighbour assembly below) is
/// identical for both modes; they differ only in where the new point rings sit:
/// [`InteriorInsert`](LayerMode::InteriorInsert) shrinks the wall points inward
/// and lands the outermost ring back on the original boundary, while
/// [`ExtrudeOutward`](LayerMode::ExtrudeOutward) leaves every existing point put
/// and grows the rings outward along the wall normal. This is pure
/// geometry+connectivity assembly; the caller quality- and watertightness-gates
/// the result.
fn build_layer_topology(
    mesh: &CastellatedMesh,
    wall_patch: usize,
    n: usize,
    controls: &LayerControls,
    mode: LayerMode,
) -> PolyPatchMesh {
    let topo = &mesh.topology;
    let base_points = &topo.points;
    let np = base_points.len();
    let nc = topo.n_cells;
    let nif = topo.n_internal_faces;

    // Wall faces (global ids) and their owner cells.
    let wface_ids: Vec<usize> = topo.patch_face_ids(wall_patch).collect();
    let nwf = wface_ids.len();
    let owner_cell: Vec<usize> = wface_ids.iter().map(|&f| topo.owner[f]).collect();

    // Deduplicated wall points and a global→local map.
    let wall_pts = topo.patch_point_ids(wall_patch);
    let nwp = wall_pts.len();
    let mut g2l: HashMap<usize, usize> = HashMap::with_capacity(nwp);
    for (l, &g) in wall_pts.iter().enumerate() {
        g2l.insert(g, l);
    }

    // Base finite-volume geometry of the ORIGINAL mesh — needed for the
    // medial-axis-style shrink cap (owner-cell depth normal to the wall).
    let (base_areas, base_centres) = topo.face_geometry();
    let (_base_ctrs, base_vols) = topo.cell_geometry(&base_areas, &base_centres);

    // Per-wall-point shrink/insert direction = area-weighted mean of incident
    // wall-face OUTWARD normals, normalised. Also track, per point, the local
    // characteristic size (min incident face √area) for the face-size cap and a
    // medial-axis distance proxy `medial_depth` = the smallest incident
    // owner-cell depth normal to the wall (`owner cell volume / wall-face area`).
    // This proxies `medialDist_` in `medialAxisMeshMover.C:1766`: the distance
    // from the wall into the fluid before the shrink would collapse the owner.
    let mut pn = vec![Vector3::ZERO; nwp];
    let mut charlen = vec![f64::INFINITY; nwp];
    let mut medial_depth = vec![f64::INFINITY; nwp];
    for (j, &fid) in wface_ids.iter().enumerate() {
        let (area, _c) = face_area_and_centre(&topo.faces[fid], base_points);
        let amag = area.mag();
        let size = amag.sqrt();
        let depth = if amag > 1e-300 {
            base_vols[owner_cell[j]] / amag
        } else {
            0.0
        };
        for &g in &topo.faces[fid] {
            if let Some(&l) = g2l.get(&g) {
                pn[l] += area;
                if size < charlen[l] {
                    charlen[l] = size;
                }
                if depth < medial_depth[l] {
                    medial_depth[l] = depth;
                }
            }
        }
    }
    for d in pn.iter_mut() {
        *d = d.normalise(1e-300);
    }

    // Graded layer thicknesses [t, t·r, …] and the total block thickness T.
    let mut thick = layer_thicknesses(controls);
    thick.truncate(n);
    let total: f64 = thick.iter().sum();

    // Per-point block scale. Both modes honour the face-size cap
    // (max_thickness_fraction·√A, the convex-corner self-intersection guard).
    // The interior insert additionally honours the medial-axis depth cap
    // (medial_axis_thickness_ratio·owner-depth) so the inward shrink cannot
    // invert the owner cell; the outward extrusion does not compress the owner,
    // so that cap does not apply to it.
    let mut pscale = vec![1.0f64; nwp];
    for l in 0..nwp {
        let cap_face = controls.max_thickness_fraction * charlen[l];
        let cap = match mode {
            LayerMode::InteriorInsert => {
                cap_face.min(controls.medial_axis_thickness_ratio * medial_depth[l])
            }
            LayerMode::ExtrudeOutward => cap_face,
        };
        if total > cap && total > 0.0 {
            pscale[l] = cap / total;
        }
    }

    // ── Points ────────────────────────────────────────────────────────────────
    // Both modes keep the same ring indexing (ring 0 = the original wall point,
    // rings 1..=n appended as `np + l*n + (k-1)`); only the coordinates differ.
    //
    // InteriorInsert: shrink the wall points inward and lay the rings back out to
    //   the ORIGINAL wall position. `dwall[k]` is the inward distance of ring k
    //   from the original wall (dwall[n] = 0, at the wall); the reversed
    //   cumulative sum puts the THINNEST layer (t) adjacent to the wall and ring
    //   n lands exactly on the untouched boundary, so the outer envelope does not
    //   move.
    // ExtrudeOutward: leave every existing point put and push the rings outward
    //   along the wall normal with the forward cumulative offset s[k], thinnest
    //   layer at the (original) wall.
    let mut points = base_points.clone();
    points.reserve(nwp * n);
    for l in 0..nwp {
        let base = base_points[wall_pts[l]];
        let ps = pscale[l];
        match mode {
            LayerMode::InteriorInsert => {
                // dwall[k] = dwall[k+1] + ps·thick[n-1-k]; outermost (wall-
                // adjacent) layer thickness is ps·thick[0] = ps·t.
                let mut dwall = vec![0.0f64; n + 1];
                for k in (0..n).rev() {
                    dwall[k] = dwall[k + 1] + ps * thick[n - 1 - k];
                }
                // Move the original wall point inward to ring 0 (shrunk interface).
                points[wall_pts[l]] = base - pn[l] * dwall[0];
                for &d in &dwall[1..=n] {
                    points.push(base - pn[l] * d);
                }
            }
            LayerMode::ExtrudeOutward => {
                // s[k] = s[k-1] + ps·thick[k-1]; grown outward, base point kept.
                let mut s = 0.0f64;
                for k in 1..=n {
                    s += ps * thick[k - 1];
                    points.push(base + pn[l] * s);
                }
            }
        }
    }

    // Local corner indices of each wall face (order preserved).
    let local_corners: Vec<Vec<usize>> = wface_ids
        .iter()
        .map(|&f| topo.faces[f].iter().map(|g| g2l[g]).collect())
        .collect();

    // Classify wall-patch edges: undirected edge → occurrences (face j, corner i).
    let mut edges: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (j, &fid) in wface_ids.iter().enumerate() {
        let f = &topo.faces[fid];
        let m = f.len();
        for i in 0..m {
            let a = f[i];
            let b = f[(i + 1) % m];
            let key = (a.min(b), a.max(b));
            edges.entry(key).or_default().push((j, i));
        }
    }

    // ── Faces (internal first, then boundary patches) ─────────────────────────
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut neighbour: Vec<usize> = Vec::new();

    // (a) original internal faces, unchanged.
    for f in 0..nif {
        faces.push(topo.faces[f].clone());
        owner.push(topo.owner[f]);
        neighbour.push(topo.neighbour[f]);
    }
    // (b) original wall faces become internal: owner cell → first prism.
    for j in 0..nwf {
        faces.push(topo.faces[wface_ids[j]].clone());
        owner.push(owner_cell[j]);
        neighbour.push(prism_cell(nc, n, j, 1));
    }
    // (c) prism-to-prism caps (ring k between prism k and k+1).
    for j in 0..nwf {
        let lc = &local_corners[j];
        for k in 1..n {
            let face: Vec<usize> = lc.iter().map(|&l| ring_pt(np, n, &wall_pts, l, k)).collect();
            faces.push(face);
            owner.push(prism_cell(nc, n, j, k));
            neighbour.push(prism_cell(nc, n, j, k + 1));
        }
    }
    // (d) side faces on shared edges → internal (owner→neighbour wound).
    for occ in edges.values() {
        if occ.len() != 2 {
            continue;
        }
        let (j0, i0) = occ[0];
        let (j1, _i1) = occ[1];
        let f0 = &topo.faces[wface_ids[j0]];
        let m0 = f0.len();
        let la = g2l[&f0[i0]];
        let lb = g2l[&f0[(i0 + 1) % m0]];
        for k in 1..=n {
            faces.push(side_quad(np, n, &wall_pts, la, lb, k));
            owner.push(prism_cell(nc, n, j0, k));
            neighbour.push(prism_cell(nc, n, j1, k));
        }
    }
    let n_internal_new = faces.len();

    // ── Boundary faces + patches ──────────────────────────────────────────────
    let mut patches: Vec<BoundaryPatch> = Vec::new();
    let mut start = n_internal_new;

    // Original non-wall patches, preserved.
    for (pi, p) in topo.patches.iter().enumerate() {
        if pi == wall_patch {
            continue;
        }
        for f in p.start..p.end() {
            faces.push(topo.faces[f].clone());
            owner.push(topo.owner[f]);
        }
        patches.push(BoundaryPatch::new(p.name.clone(), start, p.size, p.kind));
        start += p.size;
    }

    // New wall patch = outer caps (ring n) of every prism stack.
    let wall_name = topo.patches[wall_patch].name.clone();
    let wall_kind = topo.patches[wall_patch].kind;
    for j in 0..nwf {
        let lc = &local_corners[j];
        let face: Vec<usize> = lc.iter().map(|&l| ring_pt(np, n, &wall_pts, l, n)).collect();
        faces.push(face);
        owner.push(prism_cell(nc, n, j, n));
    }
    patches.push(BoundaryPatch::new(wall_name, start, nwf, wall_kind));
    start += nwf;

    // Rim side faces (edges owned by a single wall face) → `layerSide` patch.
    let mut rim_count = 0usize;
    for occ in edges.values() {
        if occ.len() != 1 {
            continue;
        }
        let (j, i) = occ[0];
        let f = &topo.faces[wface_ids[j]];
        let m = f.len();
        let la = g2l[&f[i]];
        let lb = g2l[&f[(i + 1) % m]];
        for k in 1..=n {
            faces.push(side_quad(np, n, &wall_pts, la, lb, k));
            owner.push(prism_cell(nc, n, j, k));
            rim_count += 1;
        }
    }
    if rim_count > 0 {
        patches.push(BoundaryPatch::new("layerSide", start, rim_count, PatchKind::Wall));
    }

    PolyPatchMesh {
        points,
        faces,
        owner,
        neighbour,
        n_internal_faces: n_internal_new,
        n_cells: nc + nwf * n,
        patches,
    }
}

/// Side-quad of one prism layer `k` along the wall edge (`la`→`lb`), wound so
/// its right-hand normal points laterally OUTWARD from `la`/`lb`'s owning prism
/// (i.e. owner→neighbour for a shared edge). Corners: inner_la, inner_lb,
/// outer_lb, outer_la.
#[inline]
fn side_quad(np: usize, n: usize, wall_pts: &[usize], la: usize, lb: usize, k: usize) -> Vec<usize> {
    vec![
        ring_pt(np, n, wall_pts, la, k - 1),
        ring_pt(np, n, wall_pts, lb, k - 1),
        ring_pt(np, n, wall_pts, lb, k),
        ring_pt(np, n, wall_pts, la, k),
    ]
}

/// The new wall patch's quadrilateral faces as [`SurfaceFace`] records (owner
/// prism cell + 4 corner point ids). Non-quad wall faces are skipped.
fn new_wall_surface_faces(
    topo: &PolyPatchMesh,
    wall_patch: usize,
    original: &CastellatedMesh,
) -> Vec<SurfaceFace> {
    let name = &original.topology.patches[wall_patch].name;
    // The rebuilt wall patch carries the same name; find it.
    let Some(pi) = topo.patches.iter().position(|p| &p.name == name) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for f in topo.patches[pi].start..topo.patches[pi].end() {
        let face = &topo.faces[f];
        if face.len() == 4 {
            out.push(SurfaceFace {
                owner_cell: topo.owner[f],
                corners: [face[0], face[1], face[2], face[3]],
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snappy_hex_mesh::MeshQuality;

    /// `p(ix,iy,iz)` index into the 3×2×2 corner grid used by [`two_cell_mesh`].
    fn pidx(ix: usize, iy: usize, iz: usize) -> usize {
        ix * 4 + iy * 2 + iz
    }

    /// Two unit cubes side by side in `x` (`[0,1]×[0,1]×[0,1]` and
    /// `[1,2]×[0,1]×[0,1]`). Their shared `x = 1` face is internal; the two
    /// `z = 1` top faces form a 2-quad wall patch ("surface"); all other
    /// boundary faces go in one "outer" patch. Wound per the `PolyPatchMesh`
    /// convention (internal owner→neighbour, boundary outward).
    fn two_cell_mesh() -> CastellatedMesh {
        // 12 grid points.
        let mut points = Vec::with_capacity(12);
        for ix in 0..3 {
            for iy in 0..2 {
                for iz in 0..2 {
                    points.push(Vector3::new(ix as f64, iy as f64, iz as f64));
                }
            }
        }
        let p = pidx;

        // Internal face x=1 (owner 0 → neighbour 1, normal +x).
        let mut faces: Vec<Vec<usize>> = vec![vec![p(1, 0, 0), p(1, 1, 0), p(1, 1, 1), p(1, 0, 1)]];
        let mut owner: Vec<usize> = vec![0];
        let neighbour: Vec<usize> = vec![1];

        // Outer boundary faces (8), all outward-wound.
        let outer: Vec<(Vec<usize>, usize)> = vec![
            // cell 0
            (vec![p(0, 0, 0), p(0, 0, 1), p(0, 1, 1), p(0, 1, 0)], 0), // x=0 (-x)
            (vec![p(0, 0, 0), p(1, 0, 0), p(1, 0, 1), p(0, 0, 1)], 0), // y=0 (-y)
            (vec![p(0, 1, 0), p(0, 1, 1), p(1, 1, 1), p(1, 1, 0)], 0), // y=1 (+y)
            (vec![p(0, 0, 0), p(0, 1, 0), p(1, 1, 0), p(1, 0, 0)], 0), // z=0 (-z)
            // cell 1
            (vec![p(2, 0, 0), p(2, 1, 0), p(2, 1, 1), p(2, 0, 1)], 1), // x=2 (+x)
            (vec![p(1, 0, 0), p(2, 0, 0), p(2, 0, 1), p(1, 0, 1)], 1), // y=0 (-y)
            (vec![p(1, 1, 0), p(1, 1, 1), p(2, 1, 1), p(2, 1, 0)], 1), // y=1 (+y)
            (vec![p(1, 0, 0), p(1, 1, 0), p(2, 1, 0), p(2, 0, 0)], 1), // z=0 (-z)
        ];
        for (f, o) in outer {
            faces.push(f);
            owner.push(o);
        }
        // Wall (top z=1) faces, outward +z.
        let wall: Vec<(Vec<usize>, usize)> = vec![
            (vec![p(0, 0, 1), p(1, 0, 1), p(1, 1, 1), p(0, 1, 1)], 0),
            (vec![p(1, 0, 1), p(2, 0, 1), p(2, 1, 1), p(1, 1, 1)], 1),
        ];
        for (f, o) in wall {
            faces.push(f);
            owner.push(o);
        }

        let patches = vec![
            BoundaryPatch::new("outer", 1, 8, PatchKind::Patch),
            BoundaryPatch::new("surface", 9, 2, PatchKind::Wall),
        ];

        let topology = PolyPatchMesh {
            points: points.clone(),
            faces,
            owner,
            neighbour,
            n_internal_faces: 1,
            n_cells: 2,
            patches,
        };
        let fv_mesh = topology.build_fvmesh().expect("base two-cell mesh valid");
        CastellatedMesh {
            fv_mesh,
            points,
            topology,
            surface_faces: Vec::new(),
            cells_by_level: vec![2],
            max_level: 0,
        }
    }

    /// Per-cell signed sum of face area vectors [m²]; every entry must be ~0 for
    /// a watertight (closed) cell.
    fn cell_area_sums(topo: &PolyPatchMesh) -> Vec<Vector3> {
        let (areas, _c) = topo.face_geometry();
        let mut sums = vec![Vector3::ZERO; topo.n_cells];
        for f in 0..topo.n_internal_faces {
            sums[topo.owner[f]] += areas[f];
            sums[topo.neighbour[f]] += -areas[f];
        }
        for f in topo.n_internal_faces..topo.faces.len() {
            sums[topo.owner[f]] += areas[f];
        }
        sums
    }

    /// Grading arithmetic: `[t, t·r, t·r²]` and its sum, plus the
    /// `final_layer_thickness` back-solve. Measured 2026-07-17:
    /// `[0.1, 0.15, 0.225]`, total `0.475`; `final = 0.225 ⇒ first = 0.1`.
    #[test]
    fn grading_arithmetic() {
        let c = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.1,
            ..Default::default()
        };
        let t = layer_thicknesses(&c);
        assert!((t[0] - 0.1).abs() < 1e-15);
        assert!((t[1] - 0.15).abs() < 1e-15);
        assert!((t[2] - 0.225).abs() < 1e-15);
        assert!((total_layer_thickness(&c) - 0.475).abs() < 1e-15);

        // final_layer_thickness spec recovers first = final / r^(n-1).
        let c2 = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.0, // ignored
            final_layer_thickness: Some(0.225),
            ..Default::default()
        };
        assert!((c2.first_thickness() - 0.1).abs() < 1e-12);
        let t2 = layer_thicknesses(&c2);
        assert!((t2[0] - 0.1).abs() < 1e-12);
        assert!((t2[2] - 0.225).abs() < 1e-12);
    }

    /// Interior shrink-and-insert V&V on the flat two-cell wall (fluid on
    /// `z < 1`, wall at `z = 1`). Measured 2026-07-20:
    /// old n_cells 2 → new 8 (3 per wall face); **outer boundary fixed** at
    /// `max z = 1.0` (an outward extrusion would reach 1.475); owner cells
    /// shrunk to top `z = 0.525`; layer heights from the wall inward
    /// [0.100, 0.150, 0.225] (thinnest 0.100 at the wall), ratios 1.5000 &
    /// 1.5000; min_cell_volume 0.100; 0 negative-volume cells; watertight
    /// (max |ΣSf| < 1e-12); default quality gate accepts.
    #[test]
    fn insert_flat_wall_interior_shrink_grading_and_quality() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 0.1,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("layers inserted");

        // Exactly n new cells per wall face: 2 + 2*3 = 8.
        assert_eq!(out.topology.n_cells, 8, "n_cells");
        assert_eq!(out.fv_mesh.n_cells, 8);
        assert_eq!(out.topology.n_cells - mesh.topology.n_cells, 2 * 3);

        // Rebuilt mesh validates.
        out.fv_mesh.validate().expect("layered mesh validates");

        // KEY DISTINCTION from the old extrude-outward primitive: the layers are
        // inserted INSIDE the domain, so the outer wall boundary does NOT move
        // outward. No point exceeds the original wall plane z = 1.
        let max_z = out
            .topology
            .points
            .iter()
            .map(|p| p.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(max_z <= 1.0 + 1e-12, "outer boundary moved outward: max z {max_z}");
        assert!((max_z - 1.0).abs() < 1e-12, "wall no longer at z=1: {max_z}");

        // Quality: no negative/degenerate cells, gate accepts.
        let q: MeshQuality = out.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "neg-vol cells");
        assert!(q.min_cell_volume > 0.0, "min vol {}", q.min_cell_volume);
        // Smallest prism = 1 m² base × thinnest (wall-adjacent) thickness 0.1 m.
        assert!((q.min_cell_volume - 0.1).abs() < 1e-9, "min vol {}", q.min_cell_volume);
        assert!(
            QualityLimits::default().accepts(&q),
            "quality gate rejected: {q:?}"
        );

        // Column above corner (x≈0, y≈0): gather all point z above z=0, sorted.
        // Expect the shrunk owner top plus the three ring boundaries:
        // [0.525 (owner top), 0.75, 0.90, 1.00 (wall)].
        let mut col: Vec<f64> = out
            .topology
            .points
            .iter()
            .filter(|p| p.x.abs() < 1e-9 && p.y.abs() < 1e-9 && p.z > 1e-9)
            .map(|p| p.z)
            .collect();
        col.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(col.len(), 4, "owner top + 3 ring points in the column: {col:?}");

        // Owner cell compressed to z ∈ [0, 0.525] (= 1 - total 0.475).
        assert!((col[0] - 0.525).abs() < 1e-12, "owner top {}", col[0]);
        // Outermost ring sits exactly on the untouched wall.
        assert!((col[3] - 1.0).abs() < 1e-12, "wall ring {}", col[3]);

        // Layer heights measured from the WALL inward: thinnest first.
        let h_wall = col[3] - col[2]; // wall-adjacent layer
        let h_mid = col[2] - col[1];
        let h_owner = col[1] - col[0]; // owner-adjacent layer
        assert!((h_wall - 0.1).abs() < 1e-12, "wall-adjacent h {h_wall}");
        assert!((h_mid - 0.15).abs() < 1e-12, "mid h {h_mid}");
        assert!((h_owner - 0.225).abs() < 1e-12, "owner-adjacent h {h_owner}");
        // Expansion ratio 1.5 going away from the wall.
        assert!((h_mid / h_wall - 1.5).abs() < 1e-12, "ratio {}", h_mid / h_wall);
        assert!((h_owner / h_mid - 1.5).abs() < 1e-12, "ratio {}", h_owner / h_mid);

        // Watertightness: every cell's signed face-area-vector sum ~ 0. (The
        // side rim faces + shrunk owner side faces still tile the original
        // domain boundary, so the envelope stays closed.)
        let sums = cell_area_sums(&out.topology);
        let max_res = sums.iter().map(|v| v.mag()).fold(0.0f64, f64::max);
        assert!(max_res < 1e-12, "max |ΣSf| = {max_res}");

        // The old wall faces are now internal (owner→first prism, at the shrunk
        // interface), and a new "surface" wall patch caps the block back at the
        // original boundary.
        assert!(out.topology.n_internal_faces > mesh.topology.n_internal_faces);
        let wall = out
            .topology
            .patches
            .iter()
            .find(|p| p.name == "surface")
            .expect("new surface patch");
        assert_eq!(wall.size, 2, "new wall caps");
        assert!(matches!(wall.kind, PatchKind::Wall));
        // surface_faces record the two new quad caps.
        assert_eq!(out.surface_faces.len(), 2);
    }

    /// The face-size + medial-axis caps prevent inversion even for an absurdly
    /// large requested thickness: the inward shrink and inserted block are both
    /// scaled down, so the owner cell keeps positive volume and the mesh has
    /// zero negative-volume cells. Measured 2026-07-20: requested block total
    /// 475 m; owner depth 1 m (`vol/area`) with medial ratio 0.5 and face cap
    /// 0.5·√A = 0.5 both bind ⇒ block capped to 0.5 m per point, owner top
    /// shrinks to z ≈ 0.5, min_cell_volume ≈ 0.105 m³ > 0, max z ≤ 1.
    #[test]
    fn oversized_thickness_capped_no_inversion() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 3,
            expansion_ratio: 1.5,
            first_layer_thickness: 100.0, // block 475 m, way past the 0.5 caps
            max_thickness_fraction: 0.5,
            medial_axis_thickness_ratio: 0.5,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("layers inserted (capped)");
        let q = out.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "capped: neg-vol cells");
        assert!(q.min_cell_volume > 0.0, "capped: min vol {}", q.min_cell_volume);
        // The shrink is capped, so the outer boundary still does not move out.
        let max_z = out
            .topology
            .points
            .iter()
            .map(|p| p.z)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(max_z <= 1.0 + 1e-12, "capped: outer boundary moved: {max_z}");
        out.fv_mesh.validate().expect("capped mesh validates");
    }

    /// Build a small triangulated sphere (as [`TriangleSoup`]) for the carved
    /// mesh used by the fallback V&V test.
    fn unit_sphere_soup(n_lat: usize, n_lon: usize) -> crate::snappy_hex_mesh::stl::TriangleSoup {
        use crate::snappy_hex_mesh::stl::{Triangle, TriangleSoup};
        let point = |ia: f64, io: f64| -> Vector3 {
            let theta = std::f64::consts::PI * ia;
            let phi = 2.0 * std::f64::consts::PI * io;
            Vector3::new(theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos())
        };
        let mut tris = Vec::new();
        for i in 0..n_lat {
            for j in 0..n_lon {
                let (a0, a1) = (i as f64 / n_lat as f64, (i + 1) as f64 / n_lat as f64);
                let (o0, o1) = (j as f64 / n_lon as f64, (j + 1) as f64 / n_lon as f64);
                let (p00, p01, p10, p11) =
                    (point(a0, o0), point(a0, o1), point(a1, o0), point(a1, o1));
                tris.push(Triangle::new(p00, p11, p01));
                tris.push(Triangle::new(p00, p10, p11));
            }
        }
        TriangleSoup::new("sphere", tris)
    }

    /// V&V of the mode selection on a REFINED (hanging-node) carved mesh — the
    /// exact boundary where interior insertion breaks and the outward-extrude
    /// fallback must take over.
    ///
    /// **Methodology.** Carve a unit sphere out of a `[-2,2]³` box at level 1
    /// (`castellate`), giving a mesh with coarse/fine octree interfaces whose
    /// wall points include hanging nodes. `n = 2`, `t = 5e-3 m`, `r = 1.3`.
    /// The private builder is called in each mode directly: the interior insert
    /// is expected to be NON-watertight (a moved hanging-node wall point opens a
    /// coarse/fine gap), while the outward extrusion is watertight; `add_layers`
    /// must therefore return the watertight extrude fallback, with more cells
    /// than the input and no negative-volume cell.
    ///
    /// **Results (measured 2026-07-20).** Base carved mesh `848` cells, watertight
    /// (`max |ΣSf| = 0`). Interior-insert candidate closure `max |ΣSf| ≈ 2.0e-3
    /// m²` (48 unclosed coarse cells) — NOT watertight, correctly rejected.
    /// Outward-extrude candidate closure `< 1e-9 m²` — watertight. `add_layers`
    /// returns `1472` cells (fallback taken), watertight, `n_negative = 0`,
    /// `min_cell_volume > 0`, rebuilt `FvMesh` validates.
    #[test]
    fn refined_mesh_falls_back_to_outward_extrude() {
        use crate::snappy_hex_mesh::background::{BackgroundMesh, Bounds};
        use crate::snappy_hex_mesh::castellation::{castellate, CastellationControls};

        let surface = unit_sphere_soup(16, 16);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let ctrl = CastellationControls::new(bg, 1, Vector3::new(-1.9, -1.9, -1.9));
        let cast = castellate(&surface, &ctrl).expect("castellation");
        let scale = max_face_area(&cast.topology);

        // Base carved mesh is watertight.
        assert!(
            max_cell_closure(&cast.topology) <= 1e-9 * scale,
            "base carved mesh not watertight"
        );

        let controls = LayerControls {
            n_surface_layers: 2,
            expansion_ratio: 1.3,
            first_layer_thickness: 5e-3,
            ..Default::default()
        };
        let wp = find_wall_patch(&cast.topology).expect("wall patch");

        // The interior insert is NOT watertight here (hanging-node wall points):
        // this is the documented boundary of the interior-insert method.
        let interior = build_layer_topology(&cast, wp, 2, &controls, LayerMode::InteriorInsert);
        assert!(
            max_cell_closure(&interior) > 1e-6 * scale,
            "interior insert unexpectedly watertight on refined mesh"
        );
        assert!(
            !layered_topology_acceptable(&interior, &controls),
            "interior insert should be rejected on refined mesh"
        );

        // The outward extrusion IS watertight and is accepted.
        let extrude = build_layer_topology(&cast, wp, 2, &controls, LayerMode::ExtrudeOutward);
        assert!(
            max_cell_closure(&extrude) <= 1e-9 * max_face_area(&extrude),
            "outward extrusion not watertight"
        );
        assert!(layered_topology_acceptable(&extrude, &controls));

        // add_layers takes the fallback: more cells, watertight, no inversion.
        let out = add_layers(&cast, &controls).expect("layers added via fallback");
        assert!(out.topology.n_cells > cast.topology.n_cells, "prisms added");
        assert!(
            max_cell_closure(&out.topology) <= 1e-9 * max_face_area(&out.topology),
            "final mesh not watertight"
        );
        let q = out.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "no negative cells");
        assert!(q.min_cell_volume > 0.0, "min vol {}", q.min_cell_volume);
        out.fv_mesh.validate().expect("fallback mesh validates");
    }

    /// Zero requested layers is an identity operation.
    #[test]
    fn zero_layers_identity() {
        let mesh = two_cell_mesh();
        let controls = LayerControls {
            n_surface_layers: 0,
            ..Default::default()
        };
        let out = add_layers(&mesh, &controls).expect("identity");
        assert_eq!(out.topology.n_cells, mesh.topology.n_cells);
    }

    /// A mesh with no wall patch is a construction error.
    #[test]
    fn no_wall_patch_errors() {
        let mut mesh = two_cell_mesh();
        // Downgrade the wall patch to a plain patch.
        mesh.topology.patches[1] =
            BoundaryPatch::new("surface", 9, 2, PatchKind::Patch);
        let err = add_layers(&mesh, &LayerControls::default());
        assert!(matches!(err, Err(MeshError::Construction(_))));
    }
}
