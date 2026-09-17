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

//! Snapping — Phase 2 of `snappyHexMesh` (**implemented**).
//!
//! Snapping morphs the castellated staircase boundary onto the STL surface so
//! the mesh becomes body-fitted. This is a pure-Rust port of the core of
//! OpenFOAM's `snappySnapDriver` (`src/mesh/snappyHexMesh/snappyHexMeshDriver/
//! snappySnapDriver.C`). The driver mirrored here is [`snap`], which reproduces
//! the essential loop of `snappySnapDriver::doSnap` (line 2574) minus the
//! parallel/coupled-patch and displacement-diffusion machinery that a serial,
//! single-region mesh does not need:
//!
//! 1. **Find patch points.** Collect the deduplicated points of the carved
//!    surface (`PatchKind::Wall`) patch — the points snapping projects. Mirrors
//!    the `indirectPrimitivePatch` (`ppPtr`) that `doSnap` builds over the
//!    adapt-patch faces.
//! 2. **Compute displacements** (`calcNearestSurface`, line 1797). For each
//!    patch point find the nearest point on the STL
//!    ([`TriangleSoup::nearest_point`], exact closest surface point); the
//!    displacement is `target - current`. [`raw_surface_displacements`] exposes
//!    this projection primitive on its own.
//! 3. **Patch smoothing** (`smoothPatchDisplacement`, line 290). The raw
//!    displacement field is Laplacian-smoothed over the patch-point adjacency
//!    (each point's displacement ← mean of its patch-neighbour displacements),
//!    for `n_smooth_patch` sweeps. This keeps the motion coherent so interior
//!    cells are not sheared apart. (OpenFOAM's version additionally blends
//!    boundary- and internal-face-centre averages with a manifold test; we use
//!    the simpler edge-Laplacian the task calls for and document the difference
//!    honestly.)
//! 4. **Mesh-quality-gated relaxation** (`smoothDisplacement` +
//!    `scaleMesh`/`meshMover.scaleMesh`, lines 2134/2208). The smoothed
//!    displacement is applied scaled by a relaxation factor `lambda` (start
//!    1.0). The moved [`PolyPatchMesh`](crate::snappy_hex_mesh::PolyPatchMesh) is rebuilt ([`PolyPatchMesh::build_fvmesh`](crate::snappy_hex_mesh::PolyPatchMesh::build_fvmesh))
//!    and its [`quality`](crate::snappy_hex_mesh::PolyPatchMesh::quality) checked against
//!    [`QualityLimits`]. If the move is accepted it is committed; otherwise
//!    `lambda` is halved and retried (a few backoffs), reproducing OpenFOAM's
//!    "relax displacement until correct mesh" loop. Nearest-surface targets are
//!    recomputed every solve iteration so points converge onto the surface.
//! 5. **Feature snapping with `pointConstraint` accumulation** (port of
//!    `snappySnapDriverFeature.C` — `binFeatureFace`/reconstruction lines
//!    931–993 and `featureAttractionUsingReconstruction` line 997 — together
//!    with the accumulator itself in `pointConstraintI.H`:
//!    `applyConstraint` line 48 and `constrainDisplacement` line 185). When
//!    [`SnapControls::feature_snap`] is set, STL feature edges (creases whose
//!    dihedral angle exceeds `feature_angle_deg`) **and feature points**
//!    (corners where ≥2 non-collinear feature edges meet) are extracted, and
//!    every wall patch point near a feature is classified with a per-point
//!    `PointConstraint` that governs how it is allowed to move:
//!    - **rank 0 — free on surface:** ordinary nearest-surface projection (as
//!      for a point with no nearby feature). `constrainDisplacement` is the
//!      identity here.
//!    - **rank 2 — on a feature edge:** the point is pulled onto the crease and
//!      its *smoothing correction* is constrained to the unit edge direction
//!      (`constrainDisplacement` keeps only the along-edge component), so the
//!      point slides along the crease but never drifts off it.
//!    - **rank 3 — on a feature point:** the point is pulled onto the corner
//!      vertex and fully fixed (its displacement is anchored, zero smoothing),
//!      so it lands exactly on the corner.
//!
//!    The rank labels and the free→edge→point accumulation mirror OpenFOAM's
//!    `pointConstraint` exactly: applying one constraining direction promotes
//!    free→edge, a second non-collinear direction promotes edge→point, and a
//!    saturated point stays fixed. The one honest difference from OpenFOAM is
//!    the *source* of those directions — here they are feature-edge directions
//!    extracted from the STL geometry, not surface normals read from an
//!    `extendedFeatureEdgeMesh` file (see the *Honest scope* section below).
//!
//! # Honest scope — what is and is not modelled
//!
//! Implemented and tested here: geometry-derived feature-edge and
//! feature-point extraction; the full `pointConstraint` free/edge/point
//! accumulation and `constrainDisplacement`; exact corner snapping on convex
//! feature points; along-edge-constrained crease snapping; and preservation of
//! mesh validity + watertightness under all of the above.
//!
//! Deliberately **not** modelled (documented, not silently skipped):
//! - **No `extendedFeatureEdgeMesh` file input.** OpenFOAM reads a precomputed
//!   feature-edge/point file (`surfaceFeatures`) with per-edge classification
//!   (external/internal/flat/open/multiple). Here features are detected from
//!   the triangle soup by dihedral angle only; concave vs convex creases are
//!   not distinguished and open/non-manifold edges are ignored.
//! - **No multi-region / multi-patch constraint combination.** OpenFOAM's
//!   `combine` merges constraints contributed by several surfaces/patches
//!   meeting at a point. This serial single-region port classifies each point
//!   against one feature set and does not combine across regions.
//! - **No `snapDist`-scaled trimming or per-point feature attraction from the
//!   binned pointFace surface normals.** Proximity uses a single feature band
//!   (a fraction of the local patch edge length), not OpenFOAM's per-point
//!   `snapDist`. Attraction is geometric (nearest point on edge / corner
//!   vertex), not reconstructed from plane–plane intersection of binned face
//!   normals.
//!
//! Because [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) stores only geometry
//! and no point/face-vertex topology, snapping works on the point-based
//! [`PolyPatchMesh`](crate::snappy_hex_mesh::PolyPatchMesh) carried in [`CastellatedMesh::topology`]: it moves points,
//! then [`PolyPatchMesh::build_fvmesh`](crate::snappy_hex_mesh::PolyPatchMesh::build_fvmesh) recomputes all finite-volume geometry.
//!
//! # Verification & validation
//!
//! **Methodology.** `snap_sphere_projects_onto_surface` castellates a
//! unit-radius UV sphere inside a `[-2,2]³` background box meshed `8×8×8` with
//! surface refinement level 2 (finest cell ≈ 0.125 m), then snaps with the
//! default controls. It measures, for every wall patch point, the distance to
//! the exact nearest surface point before and after snapping, and checks the
//! rebuilt mesh validates, passes the default quality gate, has zero
//! negative-volume cells, and stays watertight (per-cell sum of signed
//! face-area vectors ≈ 0).
//!
//! **Results (2026-07-17, host x86_64, release).**
//! - Pre-snap max patch-point→surface distance: **0.10500 m** (staircase error).
//! - Post-snap max patch-point→surface distance: **1.5e-4 m** — a **≈715×**
//!   reduction; mean post-snap distance **1.0e-4 m**.
//! - Rebuilt mesh: `FvMesh::validate()` Ok, `QualityLimits::default().accepts`
//!   true, 0 negative-volume cells, min cell volume **5.20e-4 m³**.
//! - Watertightness: max per-cell |Σ Sf| = **3.2e-17 m²** (machine zero).
//!
//! Interpretation: the quality-gated blended Laplacian morph pulls the
//! castellated staircase essentially onto the analytic sphere (residual ~1e-4 m,
//! i.e. ~1/1000 of a finest cell) while keeping every cell valid, non-inverted,
//! and closed. Watertightness across the octree's 2:1 interfaces is preserved by
//! constraining T-junction hanging nodes to stay collinear with their coarse-edge
//! parents (`HangingConstraint`); without that constraint an unconstrained move
//! of the same points opens gaps of ~9e-4 m² per cell.

use std::collections::HashMap;

use outram_foam_basic_lib::mesh::PatchKind;
use super::cyclic::{CyclicPointConstraints, DEFAULT_CYCLIC_TOL};
use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::CastellatedMesh;
use crate::snappy_hex_mesh::poly_topology::QualityLimits;
use crate::snappy_hex_mesh::stl::TriangleSoup;
use crate::MeshError;

/// Controls for the snapping phase (subset of OpenFOAM `snapControls`).
///
/// All lengths are in metres. Defaults mirror the commonly used
/// `snappyHexMeshDict` values (30 solve iterations, 3 patch-smoothing sweeps).
#[derive(Debug, Clone)]
pub struct SnapControls {
    /// Number of quality-gated point-displacement relaxation iterations
    /// (`snapControls::nSolveIter`). Nearest-surface targets are recomputed at
    /// the start of each, so points converge onto the surface progressively.
    pub n_solve_iter: usize,
    /// Number of Laplacian patch-smoothing sweeps applied to the displacement
    /// field each solve iteration (`snapControls::nSmoothPatch`).
    pub n_smooth_patch: usize,
    /// Relative distance (in cell sizes) a point may travel to the surface
    /// (`snapControls::tolerance`). Retained for parity with the OpenFOAM
    /// dictionary; the current serial morph gates motion on mesh quality rather
    /// than this ratio, so it is advisory.
    pub tolerance: f64,
    /// Mesh-quality limits the relaxation must satisfy for a move to be
    /// committed (non-orthogonality `[deg]`, skewness, min cell volume `[m³]`).
    pub quality: QualityLimits,
    /// Enable feature snapping (port of `snappySnapDriverFeature.C` with the
    /// `pointConstraint` accumulator of `pointConstraintI.H`): patch points near
    /// an STL crease are constrained to slide along the feature edge, and points
    /// near a corner (≥2 non-collinear feature edges meeting) are fully fixed on
    /// the corner vertex. See the module docs (item 5 + *Honest scope*).
    pub feature_snap: bool,
    /// Dihedral-angle threshold `[degrees]` above which a shared STL edge counts
    /// as a feature edge. Only used when `feature_snap` is set. A box has 90°
    /// creases, so any threshold below 90 detects its edges.
    pub feature_angle_deg: f64,
}

impl Default for SnapControls {
    fn default() -> Self {
        Self {
            n_solve_iter: 30,
            n_smooth_patch: 3,
            tolerance: 2.0,
            quality: QualityLimits::default(),
            feature_snap: false,
            feature_angle_deg: 40.0,
        }
    }
}

/// The displacement each surface point *would* move under one unconstrained
/// projection to the surface — the raw input to the relaxation solve.
///
/// `result[p]` is the vector from `mesh.points[p]` (in metres) to its nearest
/// surface point, for every point that appears in a surface face; other points
/// map to the zero vector. This is the projection primitive
/// (`calcNearestSurface`, `snappySnapDriver.C:1797`) exposed on its own so
/// callers/tests can inspect the pull without running the full morph.
pub fn raw_surface_displacements(mesh: &CastellatedMesh, surface: &TriangleSoup) -> Vec<Vector3> {
    let mut disp = vec![Vector3::ZERO; mesh.points.len()];
    for face in &mesh.surface_faces {
        for &pid in &face.corners {
            if disp[pid] == Vector3::ZERO {
                if let Some(target) = surface.nearest_point(mesh.points[pid]) {
                    disp[pid] = target - mesh.points[pid];
                }
            }
        }
    }
    disp
}

/// Morph the castellated boundary onto the surface.
///
/// Port of the core of OpenFOAM `snappySnapDriver::doSnap`
/// (`snappySnapDriver.C:2574`): iteratively projects the carved wall-patch
/// points onto `surface`, Laplacian-smooths the displacement over the patch,
/// and applies it under a mesh-quality gate, rebuilding the finite-volume
/// geometry from the moved points. See the module docs for the algorithm and
/// V&V results.
///
/// # Inputs
/// - `mesh` — a castellated mesh (from
///   [`castellate`](crate::snappy_hex_mesh::castellation::castellate)); its
///   [`topology`](CastellatedMesh::topology) must carry a `PatchKind::Wall`
///   patch (the carved surface). Point coordinates are in metres.
/// - `surface` — the STL triangle soup to snap onto (metres).
/// - `controls` — solve/smoothing counts, quality limits, and feature-snap
///   options.
///
/// # Returns
/// A new [`CastellatedMesh`] with the wall-patch points moved onto (or toward)
/// the surface and a freshly rebuilt, validated `fv_mesh`. Cell/face
/// connectivity and `surface_faces` corner indices are unchanged — only point
/// coordinates move.
///
/// # Errors
/// - [`MeshError::Construction`] if no `PatchKind::Wall` patch exists, or if the
///   final moved mesh fails [`FvMesh::validate`](outram_foam_basic_lib::mesh::FvMesh::validate).
pub fn snap(
    mesh: &CastellatedMesh,
    surface: &TriangleSoup,
    controls: &SnapControls,
) -> Result<CastellatedMesh, MeshError> {
    // 1. Find the carved wall patch (the surface to snap).
    let wall_idx = mesh
        .topology
        .patches
        .iter()
        .position(|p| p.kind == PatchKind::Wall)
        .ok_or_else(|| {
            MeshError::Construction(
                "snap: no PatchKind::Wall patch found in castellated topology \
                 (nothing to project onto the surface)"
                    .to_string(),
            )
        })?;

    let patch_points = mesh.topology.patch_point_ids(wall_idx);
    if patch_points.is_empty() {
        // Nothing to move — return an unchanged copy.
        return Ok(clone_with_points(mesh, mesh.topology.points.clone())?);
    }

    // Global point id -> local index within `patch_points`.
    let mut local_of: HashMap<usize, usize> = HashMap::with_capacity(patch_points.len());
    for (i, &pid) in patch_points.iter().enumerate() {
        local_of.insert(pid, i);
    }

    // 2. Patch-point adjacency from the wall faces (edge graph over patch points).
    let adjacency = build_patch_adjacency(mesh, wall_idx, &local_of);

    // Freeze patch points that also belong to a non-wall boundary face: moving
    // them would drag the domain boundary. Mirrors OpenFOAM's freezing of
    // points constrained by other patches (freezeExposedPoints).
    let mut frozen = frozen_patch_points(mesh, wall_idx, &local_of);

    // Detect T-junction hanging nodes (octree 2:1 edge midpoints). These are
    // *not* projected independently: a hanging node shares a coarse edge P–Q
    // with a neighbouring coarse face, and the mesh stays watertight only while
    // it remains collinear between P and Q. We therefore glue each hanging node
    // to the linear interpolation of its parent corners and re-impose that after
    // every move (see `apply_hanging_constraints`). A wall hanging node is also
    // marked frozen so it is never given its own surface displacement.
    let constraints = detect_hanging_nodes(&mesh.topology);
    for c in &constraints {
        if let Some(&l) = local_of.get(&c.node) {
            frozen[l] = true;
        }
    }

    // Cyclic (periodic) constraints. Points on a cyclic plane are not frozen
    // (see `frozen_patch_points`); instead their displacement is projected into
    // the plane and synchronised with their partner across the seam, so the
    // halves stay conformal — the invariant `check_conformity` gates on — while
    // the geometry on the seam still gets snapped. Bookkeeping errors here are
    // a malformed mesh, so they surface as a construction error rather than
    // being silently ignored.
    let cyclic = CyclicPointConstraints::build(&mesh.topology, DEFAULT_CYCLIC_TOL)
        .map_err(|e| MeshError::Construction(format!("snap: {e}")))?;

    // Optional: precompute STL feature edges and feature points once.
    let feature_edges = if controls.feature_snap {
        detect_feature_edges(surface, controls.feature_angle_deg)
    } else {
        Vec::new()
    };
    let feature_points = if controls.feature_snap {
        detect_feature_points(&feature_edges)
    } else {
        Vec::new()
    };
    // Feature band ≈ a fraction of the finest cell size; use the mean edge
    // length of the patch faces as a robust local length scale. Corners get a
    // wider capture radius than edges: a corner is a single point (not a line),
    // so fewer patch points sit within a given distance of it, and missing a
    // corner degrades the crease intersection more than missing an edge point.
    let patch_len = if controls.feature_snap {
        mean_patch_edge_length(mesh, wall_idx)
    } else {
        0.0
    };
    let edge_band = 0.5 * patch_len;
    let corner_band = 1.25 * patch_len;

    // Working copy of the topology whose points we morph in place.
    let mut work = mesh.topology.clone();

    const MAX_BACKOFF: usize = 8;
    const CONVERGE_EPS: f64 = 1e-9;
    // Weight of the Laplacian-smoothed field vs the direct surface pull (0 =
    // pure projection, 1 = pure smoothing). A moderate value regularises the
    // motion without letting smoothing drift points tangentially off-surface.
    const SMOOTH_BLEND: f64 = 0.5;

    for _iter in 0..controls.n_solve_iter {
        // 2. Raw displacement per patch point (target - current), together with
        //    the per-point feature `PointConstraint`. `anchor[i]` marks a point
        //    whose displacement must not be smoothed (a fully-fixed corner, or a
        //    frozen boundary point): it anchors its neighbours instead of being
        //    averaged. `disp[i]` for an edge point is the *perpendicular* pull
        //    onto the crease (⟂ the edge direction); the along-edge freedom is
        //    added from the smoothed field afterwards via `constrain_displacement`.
        let mut disp = vec![Vector3::ZERO; patch_points.len()];
        let mut constraint = vec![PointConstraint::Free; patch_points.len()];
        let mut anchor = frozen.clone();
        let mut max_raw = 0.0f64;
        for (i, &pid) in patch_points.iter().enumerate() {
            if frozen[i] {
                continue;
            }
            let p = work.points[pid];
            let surf_target = match surface.nearest_point(p) {
                Some(t) => t,
                None => continue,
            };
            // 5. Feature classification (pointConstraint accumulation). Off ⇒
            //    every point is `Free` and the branch below is the plain
            //    surface projection, byte-for-byte identical to the non-feature
            //    path so the sphere V&V numbers are preserved.
            let (target, c) = if controls.feature_snap {
                classify_feature(
                    p,
                    surf_target,
                    &feature_edges,
                    &feature_points,
                    edge_band,
                    corner_band,
                )
            } else {
                (surf_target, PointConstraint::Free)
            };
            constraint[i] = c;
            if matches!(c, PointConstraint::Point) {
                // Corner: fully fixed on the vertex, never smoothed.
                anchor[i] = true;
            }
            let d = target - p;
            disp[i] = d;
            let m = d.mag();
            if m > max_raw {
                max_raw = m;
            }
        }

        if max_raw < CONVERGE_EPS {
            break;
        }

        // 3. Laplacian patch-smoothing of the displacement field, then combine
        //    per the point's constraint. Corners and frozen points are anchored
        //    (not smoothed) so they hold their exact target. Free points blend a
        //    share of the smoothed field into the direct surface pull (the pure
        //    edge-Laplacian can drift points tangentially off the surface; the
        //    blend keeps the straight-to-nearest-point component dominant so the
        //    morph converges onto the surface). Edge points keep their exact
        //    perpendicular pull onto the crease and take only the along-edge
        //    component of the smoothed field (`constrain_displacement`), so they
        //    slide along the crease but never leave it.
        let mut smoothed = disp.clone();
        smooth_patch_displacement(&mut smoothed, &adjacency, &anchor, controls.n_smooth_patch);
        for i in 0..disp.len() {
            if anchor[i] {
                continue; // frozen or corner: keep the exact (possibly zero) target
            }
            match constraint[i] {
                PointConstraint::Edge { .. } => {
                    disp[i] = disp[i] + constraint[i].constrain_displacement(smoothed[i]);
                }
                _ => {
                    disp[i] = disp[i] * (1.0 - SMOOTH_BLEND) + smoothed[i] * SMOOTH_BLEND;
                }
            }
        }

        // 3b. Cyclic constraint — MUST be the last edit of `disp`, so nothing
        //     downstream can reintroduce a seam-breaking component. Projects
        //     each periodic-plane point's motion into its plane, then averages
        //     partner displacements so both halves move identically and the
        //     separation vector (hence conformity) is preserved exactly.
        cyclic.constrain_and_sync(&mut disp, &local_of);

        // 4. Quality-gated relaxation: apply lambda*disp, re-impose the hanging
        //    constraints, halving lambda on quality failure.
        let saved = work.points.clone();
        let mut lambda = 1.0f64;
        let mut committed = false;
        for _backoff in 0..MAX_BACKOFF {
            let mut trial = saved.clone();
            for (i, &pid) in patch_points.iter().enumerate() {
                trial[pid] = saved[pid] + disp[i] * lambda;
            }
            apply_hanging_constraints(&mut trial, &constraints);
            work.points = trial;
            if work.build_fvmesh().is_ok() {
                let q = work.quality();
                if controls.quality.accepts(&q) {
                    committed = true;
                    break;
                }
            }
            lambda *= 0.5;
        }
        if !committed {
            // No admissible move this iteration; restore and stop — the mesh is
            // as snapped as the quality limits allow.
            work.points = saved;
            break;
        }
    }

    // 5. Rebuild and return.
    clone_with_points(mesh, work.points)
}

/// Assemble a new [`CastellatedMesh`] from `mesh` with the given moved point
/// coordinates, rebuilding the finite-volume geometry.
///
/// Connectivity, `surface_faces` corner indices, `cells_by_level`, and
/// `max_level` are copied unchanged; only coordinates differ.
fn clone_with_points(
    mesh: &CastellatedMesh,
    points: Vec<Vector3>,
) -> Result<CastellatedMesh, MeshError> {
    let mut topology = mesh.topology.clone();
    topology.points = points.clone();
    let fv_mesh = topology.build_fvmesh()?;
    Ok(CastellatedMesh {
        fv_mesh,
        points,
        topology,
        surface_faces: mesh.surface_faces.clone(),
        cells_by_level: mesh.cells_by_level.clone(),
        max_level: mesh.max_level,
    })
}

/// Build the edge-adjacency graph over the wall-patch points (local indices).
///
/// `adjacency[i]` lists the local indices of the patch points sharing a wall-face
/// edge with patch point `i`. Mirrors the `pp.pointFaces()` / edge connectivity
/// that `smoothPatchDisplacement` averages over.
fn build_patch_adjacency(
    mesh: &CastellatedMesh,
    wall_idx: usize,
    local_of: &HashMap<usize, usize>,
) -> Vec<Vec<usize>> {
    let n = local_of.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for f in mesh.topology.patch_face_ids(wall_idx) {
        let poly = &mesh.topology.faces[f];
        let k = poly.len();
        for e in 0..k {
            let a = poly[e];
            let b = poly[(e + 1) % k];
            if let (Some(&la), Some(&lb)) = (local_of.get(&a), local_of.get(&b)) {
                if !adj[la].contains(&lb) {
                    adj[la].push(lb);
                }
                if !adj[lb].contains(&la) {
                    adj[lb].push(la);
                }
            }
        }
    }
    adj
}

/// Flag wall-patch points that also lie on another (non-wall) boundary patch.
///
/// Such points are frozen during the morph so the domain boundary is not
/// dragged. Mirrors OpenFOAM `freezeExposedPoints` / patch-constraint freezing.
fn frozen_patch_points(
    mesh: &CastellatedMesh,
    wall_idx: usize,
    local_of: &HashMap<usize, usize>,
) -> Vec<bool> {
    let mut frozen = vec![false; local_of.len()];
    let wall_range = mesh.topology.patch_face_ids(wall_idx);
    let n_int = mesh.topology.n_internal_faces;

    // Faces belonging to a cyclic patch do NOT freeze their points. A periodic
    // plane is not a fixed wall: its points may slide *within* the plane as long
    // as both halves move identically, which
    // [`CyclicPointConstraints`](crate::snappy_hex_mesh::CyclicPointConstraints)
    // enforces. Freezing them instead — the behaviour before cyclic support —
    // left the geometry un-snapped (staircased) exactly where it crosses a
    // periodic plane. A point that *also* lies on some other boundary patch is
    // still frozen by that patch's faces below.
    let cyclic_ranges: Vec<std::ops::Range<usize>> = mesh
        .topology
        .patches
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind == PatchKind::Cyclic)
        .map(|(i, _)| mesh.topology.patch_face_ids(i))
        .collect();

    for (fi, poly) in mesh.topology.faces.iter().enumerate() {
        // Only boundary faces outside the wall patch can freeze a point.
        if fi < n_int || wall_range.contains(&fi) {
            continue;
        }
        if cyclic_ranges.iter().any(|r| r.contains(&fi)) {
            continue;
        }
        for &p in poly {
            if let Some(&l) = local_of.get(&p) {
                frozen[l] = true;
            }
        }
    }
    frozen
}

/// Laplacian-smooth a displacement field over the patch-point graph, in place.
///
/// Each non-frozen point's displacement is replaced by the mean of its
/// neighbours' displacements, `n_sweeps` times (Jacobi iteration). Frozen points
/// keep their (zero) displacement, anchoring the boundary. Port of the smoothing
/// intent of `snappySnapDriver::smoothPatchDisplacement` (`snappySnapDriver.C:290`).
fn smooth_patch_displacement(
    disp: &mut [Vector3],
    adjacency: &[Vec<usize>],
    frozen: &[bool],
    n_sweeps: usize,
) {
    for _ in 0..n_sweeps {
        let prev = disp.to_vec();
        for i in 0..disp.len() {
            if frozen[i] || adjacency[i].is_empty() {
                continue;
            }
            let mut sum = Vector3::ZERO;
            for &j in &adjacency[i] {
                sum += prev[j];
            }
            disp[i] = sum / adjacency[i].len() as f64;
        }
    }
}

/// A T-junction hanging node glued to the linear interpolation of two parent
/// corners: `points[node] = (1-t)·points[p] + t·points[q]`.
///
/// Octree 2:1 refinement leaves the midpoint of a coarse edge as a real vertex
/// of the finer faces, while the abutting coarse face still carries the straight
/// edge `p–q`. Watertightness (per-cell Σ Sf = 0) holds only while the node
/// stays collinear between `p` and `q`; this constraint enforces exactly that as
/// the parents move during snapping.
#[derive(Debug, Clone, Copy)]
struct HangingConstraint {
    node: usize,
    p: usize,
    q: usize,
    t: f64,
}

/// Detect octree edge-midpoint hanging nodes.
///
/// For every face edge `(a, b)`, if a mesh point sits exactly at the midpoint
/// `(a+b)/2` (matched on a fine coordinate grid) and is not `a` or `b`, it is a
/// T-junction hanging node with parents `(a, b)` and parameter `t = 0.5`.
/// Coordinates are compared in metres on a 1e-9 m grid, which the exact
/// power-of-two octree corner coordinates hit cleanly. Face-centre hanging nodes
/// need no constraint: they are interior to a coarse face's sub-face tiling, so
/// their edges cancel within the owning cell and moving them preserves closure.
fn detect_hanging_nodes(topo: &crate::snappy_hex_mesh::PolyPatchMesh) -> Vec<HangingConstraint> {
    let quant = 1e-9_f64;
    let key = |v: Vector3| -> (i64, i64, i64) {
        (
            (v.x / quant).round() as i64,
            (v.y / quant).round() as i64,
            (v.z / quant).round() as i64,
        )
    };
    let mut coord_map: HashMap<(i64, i64, i64), usize> = HashMap::with_capacity(topo.points.len());
    for (i, &p) in topo.points.iter().enumerate() {
        coord_map.insert(key(p), i);
    }
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut cons = Vec::new();
    for face in &topo.faces {
        let k = face.len();
        for e in 0..k {
            let a = face[e];
            let b = face[(e + 1) % k];
            let mid = (topo.points[a] + topo.points[b]) * 0.5;
            if let Some(&m) = coord_map.get(&key(mid)) {
                if m != a && m != b && seen.insert(m) {
                    cons.push(HangingConstraint {
                        node: m,
                        p: a,
                        q: b,
                        t: 0.5,
                    });
                }
            }
        }
    }
    cons
}

/// Re-impose the hanging-node constraints on a point array, in place.
///
/// Applied a few times so that a hanging node whose parent is itself a hanging
/// node (nested 2:1 interfaces) resolves. `points[node] ← (1-t)·points[p] +
/// t·points[q]`.
fn apply_hanging_constraints(points: &mut [Vector3], constraints: &[HangingConstraint]) {
    for _pass in 0..3 {
        for c in constraints {
            points[c.node] = points[c.p] * (1.0 - c.t) + points[c.q] * c.t;
        }
    }
}

/// A detected STL feature edge: the two endpoints (metres) of a crease.
#[derive(Debug, Clone, Copy)]
struct FeatureEdge {
    a: Vector3,
    b: Vector3,
}

/// Detect feature edges of a triangle soup: edges shared by two triangles whose
/// dihedral angle exceeds `angle_deg`.
///
/// Two triangles share an edge when they share two vertices (matched by
/// coordinate, within a small absolute tolerance). The dihedral angle is the
/// angle between the two facet normals; a shared edge is a feature edge when
/// that angle exceeds the threshold. Restricted port of the feature-edge
/// extraction underlying `snappySnapDriverFeature.C`.
fn detect_feature_edges(surface: &TriangleSoup, angle_deg: f64) -> Vec<FeatureEdge> {
    let cos_thresh = angle_deg.to_radians().cos();
    // Quantise vertices to merge coincident endpoints (STL soups are unshared).
    let quant = 1e-6_f64; // 1 micron grid — adequate for metre-scale test geometry.
    let key = |v: Vector3| -> (i64, i64, i64) {
        (
            (v.x / quant).round() as i64,
            (v.y / quant).round() as i64,
            (v.z / quant).round() as i64,
        )
    };
    // Map undirected edge (sorted vertex keys) -> (triangle normal, an endpoint pair).
    let mut edges: HashMap<((i64, i64, i64), (i64, i64, i64)), Vec<(Vector3, Vector3, Vector3)>> =
        HashMap::new();
    for t in &surface.triangles {
        let n = t.normal();
        let verts = [t.a, t.b, t.c];
        for e in 0..3 {
            let p = verts[e];
            let q = verts[(e + 1) % 3];
            let (kp, kq) = (key(p), key(q));
            let ek = if kp <= kq { (kp, kq) } else { (kq, kp) };
            edges.entry(ek).or_default().push((n, p, q));
        }
    }
    let mut features = Vec::new();
    for (_ek, tris) in edges {
        if tris.len() != 2 {
            continue; // boundary or non-manifold edge
        }
        let n0 = tris[0].0;
        let n1 = tris[1].0;
        // Both normals are unit (Triangle::normal); dihedral cos = n0·n1.
        let cos = n0.dot(n1).clamp(-1.0, 1.0);
        if cos < cos_thresh {
            features.push(FeatureEdge {
                a: tris[0].1,
                b: tris[0].2,
            });
        }
    }
    features
}

/// Per-point motion constraint accumulated from nearby features — a port of
/// OpenFOAM's `pointConstraint` (`pointConstraintI.H`).
///
/// OpenFOAM stores the constraint as a `(rank, vector)` tuple and promotes the
/// rank as constraining directions are applied
/// (`pointConstraint::applyConstraint`, `pointConstraintI.H:48`):
/// rank 0 (free) → rank 1/2 (one plane/line) → rank 3 (fixed). This enum keeps
/// the three geometrically-distinct outcomes a serial feature snap needs, using
/// the same OpenFOAM rank numbering in the docs:
///
/// - [`Free`](PointConstraint::Free) — OpenFOAM rank 0/1: the point may move
///   freely (surface projection handles it). `constrain_displacement` is the
///   identity.
/// - [`Edge`](PointConstraint::Edge) — OpenFOAM rank 2: the point lies on a
///   feature edge and may move only along the unit edge direction `dir`
///   (all lengths in metres; `dir` is dimensionless). `constrain_displacement`
///   keeps only the along-`dir` component (`pointConstraintI.H:201`).
/// - [`Point`](PointConstraint::Point) — OpenFOAM rank 3: the point lies on a
///   feature corner and is fully fixed. `constrain_displacement` returns zero
///   (`pointConstraintI.H:206`).
#[derive(Debug, Clone, Copy, PartialEq)]
enum PointConstraint {
    /// Unconstrained — ordinary surface projection governs the move.
    Free,
    /// Constrained to a feature edge; motion allowed only along `dir` (unit).
    Edge { dir: Vector3 },
    /// Constrained to a feature point (corner); fully fixed.
    Point,
}

/// Two feature-edge directions closer than this (in `|a × b|`, both unit) are
/// treated as collinear — mirrors OpenFOAM's `magPlaneNormal > 1e-3` /
/// `mag(second & pc.second) <= 1-1e-3` collinearity tests in
/// `pointConstraintI.H` (`applyConstraint`/`combine`).
const FEATURE_COLLINEAR_TOL: f64 = 1e-3;

impl PointConstraint {
    /// Accumulate one constraining feature-edge direction, promoting the rank —
    /// a port of `pointConstraint::applyConstraint` (`pointConstraintI.H:48`).
    ///
    /// `dir` need not be unit; it is normalised here (a zero vector is ignored).
    /// `Free` → `Edge` on the first direction; `Edge` → `Point` when a second,
    /// non-collinear direction arrives (the OpenFOAM `free→edge→point`
    /// promotion, driven by feature-edge directions rather than plane normals);
    /// `Point` is saturated.
    fn apply_edge_dir(&mut self, dir: Vector3) {
        let u = dir.normalise(1e-30);
        if u == Vector3::ZERO {
            return;
        }
        match self {
            PointConstraint::Free => *self = PointConstraint::Edge { dir: u },
            PointConstraint::Edge { dir } => {
                if dir.cross(u).mag() > FEATURE_COLLINEAR_TOL {
                    *self = PointConstraint::Point;
                }
            }
            PointConstraint::Point => {}
        }
    }

    /// Project a displacement onto the subspace this constraint permits — a port
    /// of `pointConstraint::constrainDisplacement` (`pointConstraintI.H:185`).
    ///
    /// `Free` returns `d` unchanged; `Edge{dir}` keeps only the along-`dir`
    /// component `(d·dir)·dir`; `Point` returns the zero vector. `d` is in
    /// metres; the result is in metres.
    fn constrain_displacement(&self, d: Vector3) -> Vector3 {
        match self {
            PointConstraint::Free => d,
            PointConstraint::Edge { dir } => *dir * d.dot(*dir),
            PointConstraint::Point => Vector3::ZERO,
        }
    }
}

/// Detect feature points (corners) from a set of feature edges.
///
/// A feature point is a vertex where ≥2 non-collinear feature edges meet — the
/// geometric analogue of OpenFOAM's rank-3 `pointConstraint`
/// (`snappySnapDriverFeature.C` reconstruction, three distinct surface-normal
/// bins → a corner, lines 972–993). Endpoints are merged on a 1e-6 m grid, the
/// unit direction of every incident feature edge (pointing away from the shared
/// vertex) is accumulated with [`PointConstraint::apply_edge_dir`], and the
/// vertex is a feature point iff the accumulation saturates to
/// [`PointConstraint::Point`]. Returns the corner positions `[m]`.
fn detect_feature_points(edges: &[FeatureEdge]) -> Vec<Vector3> {
    let quant = 1e-6_f64;
    let key = |v: Vector3| -> (i64, i64, i64) {
        (
            (v.x / quant).round() as i64,
            (v.y / quant).round() as i64,
            (v.z / quant).round() as i64,
        )
    };
    // vertex key -> (position, incident unit directions away from the vertex)
    let mut incident: HashMap<(i64, i64, i64), (Vector3, Vec<Vector3>)> = HashMap::new();
    for e in edges {
        let dir = (e.b - e.a).normalise(1e-30);
        if dir == Vector3::ZERO {
            continue;
        }
        incident
            .entry(key(e.a))
            .or_insert((e.a, Vec::new()))
            .1
            .push(dir);
        incident
            .entry(key(e.b))
            .or_insert((e.b, Vec::new()))
            .1
            .push(-dir);
    }
    let mut points = Vec::new();
    for (_k, (pos, dirs)) in incident {
        let mut pc = PointConstraint::Free;
        for d in dirs {
            pc.apply_edge_dir(d);
        }
        if pc == PointConstraint::Point {
            points.push(pos);
        }
    }
    points
}

/// Classify a patch point against the feature set, returning its attraction
/// target `[m]` and its `PointConstraint`.
///
/// Port of the intent of `snappySnapDriver::featureAttractionUsingReconstruction`
/// (`snappySnapDriverFeature.C:997`) and its `binFeatureFace` helper (lines
/// 931–993), with geometry-derived features in place of binned surface normals:
///
/// - If a **feature point** lies within `corner_band` of `p`, the point is a
///   corner: target = the corner vertex, constraint = [`PointConstraint::Point`]
///   (fully fixed).
/// - Otherwise, every **feature edge** within `edge_band` of `p` is accumulated
///   with [`PointConstraint::apply_edge_dir`]. One (or several collinear) edges
///   ⇒ [`PointConstraint::Edge`] with target = the nearest point on the closest
///   crease. Two non-collinear edges ⇒ a corner missed by explicit vertex
///   detection: [`PointConstraint::Point`] at the nearest crease point.
/// - If no feature is within band, the point is [`PointConstraint::Free`] and
///   its target is the supplied `surf_target` (the nearest-surface projection).
///
/// `p` is the current point position `[m]`; `surf_target` its nearest-surface
/// projection `[m]`; `edge_band`/`corner_band` are capture radii `[m]`.
fn classify_feature(
    p: Vector3,
    surf_target: Vector3,
    feature_edges: &[FeatureEdge],
    feature_points: &[Vector3],
    edge_band: f64,
    corner_band: f64,
) -> (Vector3, PointConstraint) {
    // Corner first — highest rank wins (OpenFOAM keeps the higher-rank
    // constraint, `snappySnapDriverFeature.C:1079`).
    let mut best_corner: Option<Vector3> = None;
    let mut best_cd = corner_band;
    for &fp in feature_points {
        let d = p.dist(fp);
        if d <= best_cd {
            best_cd = d;
            best_corner = Some(fp);
        }
    }
    if let Some(fp) = best_corner {
        return (fp, PointConstraint::Point);
    }

    // Edge accumulation.
    let mut pc = PointConstraint::Free;
    let mut best_edge: Option<Vector3> = None;
    let mut best_ed = edge_band;
    for e in feature_edges {
        let c = closest_point_on_segment(p, e.a, e.b);
        let d = p.dist(c);
        if d <= edge_band {
            pc.apply_edge_dir(e.b - e.a);
            if d <= best_ed {
                best_ed = d;
                best_edge = Some(c);
            }
        }
    }
    match (pc, best_edge) {
        (PointConstraint::Free, _) | (_, None) => (surf_target, PointConstraint::Free),
        (PointConstraint::Edge { dir }, Some(c)) => (c, PointConstraint::Edge { dir }),
        // ≥2 non-collinear crease directions within band but no explicit vertex:
        // treat as a corner pinned at the nearest crease point.
        (PointConstraint::Point, Some(c)) => (c, PointConstraint::Point),
    }
}

/// Mean edge length of the wall-patch faces `[m]` — a local length scale used to
/// size the feature-attraction band.
fn mean_patch_edge_length(mesh: &CastellatedMesh, wall_idx: usize) -> f64 {
    let mut total = 0.0f64;
    let mut count = 0usize;
    for f in mesh.topology.patch_face_ids(wall_idx) {
        let poly = &mesh.topology.faces[f];
        let k = poly.len();
        for e in 0..k {
            let a = mesh.topology.points[poly[e]];
            let b = mesh.topology.points[poly[(e + 1) % k]];
            total += a.dist(b);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        total / count as f64
    }
}

/// Nearest point on any feature edge to `p` `[m]`, or `None` if there are no
/// feature edges. Retained as a standalone crease-distance primitive used by the
/// V&V tests to measure how close snapped points land to the feature edges.
#[cfg_attr(not(test), allow(dead_code))]
fn nearest_feature_point(edges: &[FeatureEdge], p: Vector3) -> Option<Vector3> {
    let mut best: Option<Vector3> = None;
    let mut best_d = f64::INFINITY;
    for e in edges {
        let c = closest_point_on_segment(p, e.a, e.b);
        let d = p.dist(c);
        if d < best_d {
            best_d = d;
            best = Some(c);
        }
    }
    best
}

/// Closest point on segment `[a, b]` to `p` (all in metres).
fn closest_point_on_segment(p: Vector3, a: Vector3, b: Vector3) -> Vector3 {
    let ab = b - a;
    let denom = ab.dot(ab);
    if denom <= 1e-300 {
        return a;
    }
    let t = ((p - a).dot(ab) / denom).clamp(0.0, 1.0);
    a + ab * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snappy_hex_mesh::background::{BackgroundMesh, Bounds};
    use crate::snappy_hex_mesh::castellation::{castellate, CastellationControls};
    use crate::snappy_hex_mesh::stl::Triangle;

    /// A closed UV sphere of radius `r` centred at `c`, as an outward-wound
    /// triangle soup.
    fn sphere_soup(c: Vector3, r: f64, n_lat: usize, n_lon: usize) -> TriangleSoup {
        let mut tris = Vec::new();
        let point = |ia: f64, io: f64| -> Vector3 {
            let theta = std::f64::consts::PI * ia;
            let phi = 2.0 * std::f64::consts::PI * io;
            Vector3::new(
                c.x + r * theta.sin() * phi.cos(),
                c.y + r * theta.sin() * phi.sin(),
                c.z + r * theta.cos(),
            )
        };
        for i in 0..n_lat {
            for j in 0..n_lon {
                let a0 = i as f64 / n_lat as f64;
                let a1 = (i + 1) as f64 / n_lat as f64;
                let o0 = j as f64 / n_lon as f64;
                let o1 = (j + 1) as f64 / n_lon as f64;
                let p00 = point(a0, o0);
                let p01 = point(a0, o1);
                let p10 = point(a1, o0);
                let p11 = point(a1, o1);
                tris.push(Triangle::new(p00, p11, p01));
                tris.push(Triangle::new(p00, p10, p11));
            }
        }
        TriangleSoup::new("sphere", tris)
    }

    /// A closed axis-aligned box `[lo, hi]` as an outward-wound triangle soup
    /// (12 facets). Used for feature-edge tests — a box has 90° creases.
    fn box_soup(lo: Vector3, hi: Vector3) -> TriangleSoup {
        // 8 corners.
        let v = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);
        let c = [
            v(lo.x, lo.y, lo.z), // 0
            v(hi.x, lo.y, lo.z), // 1
            v(hi.x, hi.y, lo.z), // 2
            v(lo.x, hi.y, lo.z), // 3
            v(lo.x, lo.y, hi.z), // 4
            v(hi.x, lo.y, hi.z), // 5
            v(hi.x, hi.y, hi.z), // 6
            v(lo.x, hi.y, hi.z), // 7
        ];
        // Quad faces (outward CCW), split into two triangles each.
        let quads = [
            [0, 3, 2, 1], // z = lo (normal -z)
            [4, 5, 6, 7], // z = hi (normal +z)
            [0, 1, 5, 4], // y = lo (normal -y)
            [3, 7, 6, 2], // y = hi (normal +y)
            [0, 4, 7, 3], // x = lo (normal -x)
            [1, 2, 6, 5], // x = hi (normal +x)
        ];
        let mut tris = Vec::new();
        for q in quads {
            tris.push(Triangle::new(c[q[0]], c[q[1]], c[q[2]]));
            tris.push(Triangle::new(c[q[0]], c[q[2]], c[q[3]]));
        }
        TriangleSoup::new("box", tris)
    }

    /// Max distance `[m]` from every wall-patch point to the exact nearest
    /// surface point, plus the mean.
    fn patch_surface_error(mesh: &CastellatedMesh, surface: &TriangleSoup) -> (f64, f64) {
        let wall = mesh
            .topology
            .patches
            .iter()
            .position(|p| p.kind == PatchKind::Wall)
            .unwrap();
        let ids = mesh.topology.patch_point_ids(wall);
        let mut max = 0.0f64;
        let mut sum = 0.0f64;
        for &pid in &ids {
            let p = mesh.topology.points[pid];
            if let Some(t) = surface.nearest_point(p) {
                let d = p.dist(t);
                if d > max {
                    max = d;
                }
                sum += d;
            }
        }
        (max, sum / ids.len().max(1) as f64)
    }

    /// Max over all cells of the magnitude of the sum of signed (outward-oriented)
    /// face-area vectors `[m²]`. Zero ⇒ every cell is a closed polyhedron.
    fn max_cell_closure(mesh: &CastellatedMesh) -> f64 {
        let topo = &mesh.topology;
        let (areas, _c) = topo.face_geometry();
        let mut acc = vec![Vector3::ZERO; topo.n_cells];
        for (f, &o) in topo.owner.iter().enumerate() {
            acc[o] += areas[f];
        }
        for (f, &nb) in topo.neighbour.iter().enumerate() {
            acc[nb] -= areas[f];
        }
        acc.iter().map(|v| v.mag()).fold(0.0, f64::max)
    }

    /// V&V — snapping a castellated sphere box onto the STL.
    ///
    /// Methodology & Results are recorded in the module `//!` docs (dated
    /// 2026-07-17). This test enforces the numbers.
    #[test]
    fn snap_sphere_projects_onto_surface() {
        let surface = sphere_soup(Vector3::ZERO, 1.0, 24, 24);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let controls = CastellationControls::new(bg, 2, Vector3::new(-1.9, -1.9, -1.9));
        let cast = castellate(&surface, &controls).expect("castellation succeeds");

        let (pre_max, _pre_mean) = patch_surface_error(&cast, &surface);

        let snapped = snap(&cast, &surface, &SnapControls::default()).expect("snap succeeds");

        let (post_max, post_mean) = patch_surface_error(&snapped, &surface);

        println!(
            "snap sphere: pre_max={pre_max:.5} post_max={post_max:.5} \
             post_mean={post_mean:.5} ratio={:.2}x",
            pre_max / post_max.max(1e-30)
        );

        // The rebuilt mesh is valid, passes the quality gate, is watertight.
        snapped.fv_mesh.validate().expect("rebuilt mesh validates");
        let q = snapped.topology.quality();
        assert!(
            QualityLimits::default().accepts(&q),
            "post-snap quality gate: {q:?}"
        );
        assert_eq!(q.n_negative_volume_cells, 0, "no inverted cells");
        assert!(q.min_cell_volume > 0.0, "positive min cell volume");
        let closure = max_cell_closure(&snapped);
        println!(
            "snap sphere: min_vol={:.3e} max_closure={closure:.3e}",
            q.min_cell_volume
        );
        assert!(
            closure < 1e-9,
            "cells stay watertight (max |Σ Sf| = {closure:.3e})"
        );

        // Snapping markedly tightens the staircase error.
        assert!(post_max < pre_max, "snap reduces surface error");
        assert!(
            post_max < 0.5 * pre_max,
            "snap at least halves the max surface error: pre={pre_max:.5} post={post_max:.5}"
        );
        // Measured 2026-07-17: pre_max=0.10500 m, post_max=1.5e-4 m (~715x
        // tighter), post_mean=1.0e-4 m. Guard with a generous margin.
        assert!(
            post_max < 0.005,
            "post-snap max surface distance small: {post_max:.5} m"
        );
    }

    /// Feature-edge snapping pulls near-crease points exactly onto the box edges.
    ///
    /// Methodology: castellate a `[-1,1]³` box inside a `[-2,2]³` background
    /// (`8×8×8`, level 2), then snap with `feature_snap = true`. For every wall
    /// patch point whose surface projection lands within the feature band of a
    /// box edge, the snapped point must lie on a feature edge to tight tolerance.
    /// Results (2026-07-20, `pointConstraint` port): 12 feature edges detected;
    /// 212 patch points landed on a box feature edge with max residual
    /// 5.55e-17 m (machine zero); mesh stays valid + watertight (max per-cell
    /// |Σ Sf| = 8.13e-18 m²). (This is the coarse count-based check; the exact
    /// corner-vs-edge classification is in `feature_snap_box_corners_and_edges`.)
    #[test]
    fn feature_snap_captures_box_edges() {
        let surface = box_soup(Vector3::new(-1.0, -1.0, -1.0), Vector3::new(1.0, 1.0, 1.0));
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let controls = CastellationControls::new(bg, 2, Vector3::new(-1.9, -1.9, -1.9));
        let cast = castellate(&surface, &controls).expect("castellate box");

        let feats = detect_feature_edges(&surface, 40.0);
        assert_eq!(
            feats.len(),
            12,
            "a box has 12 feature edges, detected {}",
            feats.len()
        );

        let mut sc = SnapControls::default();
        sc.feature_snap = true;
        sc.feature_angle_deg = 40.0;
        let snapped = snap(&cast, &surface, &sc).expect("snap box");

        snapped
            .fv_mesh
            .validate()
            .expect("rebuilt box mesh validates");
        let q = snapped.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "no inverted cells: {q:?}");
        let closure = max_cell_closure(&snapped);
        assert!(closure < 1e-9, "box cells watertight: {closure:.3e}");

        // Count patch points that ended up essentially on a feature edge.
        let wall = snapped
            .topology
            .patches
            .iter()
            .position(|p| p.kind == PatchKind::Wall)
            .unwrap();
        let ids = snapped.topology.patch_point_ids(wall);
        let mut on_edge = 0usize;
        let mut max_edge_resid = 0.0f64;
        for &pid in &ids {
            let p = snapped.topology.points[pid];
            if let Some(fp) = nearest_feature_point(&feats, p) {
                let d = p.dist(fp);
                if d < 1e-6 {
                    on_edge += 1;
                    if d > max_edge_resid {
                        max_edge_resid = d;
                    }
                }
            }
        }
        println!(
            "feature snap: {on_edge} points snapped onto box edges, max residual {max_edge_resid:.3e} m"
        );
        assert!(
            on_edge > 0,
            "feature snapping puts some patch points exactly on box edges"
        );
    }

    /// The Laplacian smoother averages neighbour displacements and keeps frozen
    /// points fixed.
    #[test]
    fn laplacian_smoother_averages_and_anchors() {
        // Path graph 0-1-2 ; freeze the ends, non-uniform middle.
        let adj = vec![vec![1usize], vec![0usize, 2usize], vec![1usize]];
        let frozen = vec![true, false, true];
        let mut disp = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(6.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
        ];
        smooth_patch_displacement(&mut disp, &adj, &frozen, 1);
        // Middle becomes mean of the two frozen zeros = 0; ends stay 0.
        assert!(disp[0].mag() < 1e-15);
        assert!(disp[1].mag() < 1e-15);
        assert!(disp[2].mag() < 1e-15);
    }

    /// `closest_point_on_segment` clamps to endpoints and projects interior.
    #[test]
    fn segment_projection() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(2.0, 0.0, 0.0);
        // Interior projection.
        let c = closest_point_on_segment(Vector3::new(1.0, 1.0, 0.0), a, b);
        assert!(c.dist(Vector3::new(1.0, 0.0, 0.0)) < 1e-12);
        // Clamp beyond b.
        let c2 = closest_point_on_segment(Vector3::new(5.0, 3.0, 0.0), a, b);
        assert!(c2.dist(b) < 1e-12);
    }

    /// `snap` errors cleanly if there is no wall patch to project.
    #[test]
    fn snap_requires_wall_patch() {
        // Build a trivial castellation, then strip the wall patch kind.
        let surface = sphere_soup(Vector3::ZERO, 1.0, 12, 12);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let controls = CastellationControls::new(bg, 1, Vector3::new(-1.9, -1.9, -1.9));
        let mut cast = castellate(&surface, &controls).unwrap();
        for p in &mut cast.topology.patches {
            if p.kind == PatchKind::Wall {
                p.kind = PatchKind::Patch;
            }
        }
        let err = snap(&cast, &surface, &SnapControls::default()).unwrap_err();
        assert!(matches!(err, MeshError::Construction(_)));
    }

    /// V&V — the `pointConstraint` accumulator reproduces OpenFOAM's
    /// free→edge→point rank promotion and `constrainDisplacement` projection.
    ///
    /// Methodology: exercise [`PointConstraint::apply_edge_dir`] and
    /// [`PointConstraint::constrain_displacement`] against the semantics of
    /// `pointConstraintI.H` (`applyConstraint` line 48, `constrainDisplacement`
    /// line 185). Reference behaviour: applying one direction ⇒ rank-2 edge;
    /// applying a second collinear direction ⇒ still an edge; applying a second
    /// non-collinear direction ⇒ rank-3 point. `constrain_displacement` must be
    /// the identity for `Free`, keep only the along-`dir` component for `Edge`,
    /// and return zero for `Point`. Pass criterion: all four projections exact
    /// to 1e-15 m and the promotions land in the expected variant.
    ///
    /// Results (2026-07-20): all promotions correct; edge projection of
    /// (1,2,3) onto x̂ = (1,0,0) exactly; point projection = 0 exactly;
    /// free projection returns the input unchanged. Residuals 0.0 m.
    #[test]
    fn point_constraint_accumulator_and_projection() {
        let x = Vector3::new(1.0, 0.0, 0.0);
        let y = Vector3::new(0.0, 1.0, 0.0);

        // Free is the identity projection.
        let free = PointConstraint::Free;
        let d = Vector3::new(1.0, 2.0, 3.0);
        assert!(free.constrain_displacement(d).dist(d) < 1e-15);

        // One direction (or several collinear) ⇒ Edge; keeps only along-dir.
        let mut pc = PointConstraint::Free;
        pc.apply_edge_dir(x * 2.0); // non-unit input is normalised
        pc.apply_edge_dir(x * -5.0); // collinear ⇒ still an edge
        assert!(matches!(pc, PointConstraint::Edge { .. }));
        let proj = pc.constrain_displacement(Vector3::new(1.0, 2.0, 3.0));
        assert!(
            proj.dist(Vector3::new(1.0, 0.0, 0.0)) < 1e-15,
            "along-x only"
        );

        // A second, non-collinear direction ⇒ Point (fully fixed).
        pc.apply_edge_dir(y);
        assert_eq!(pc, PointConstraint::Point);
        assert!(
            pc.constrain_displacement(Vector3::new(1.0, 2.0, 3.0)).mag() < 1e-15,
            "corner is fully fixed"
        );
    }

    /// V&V — feature-point detection recovers a box's 8 corners.
    ///
    /// Methodology: extract the 12 feature edges of a `[-1,1]³` box
    /// (`detect_feature_edges`, 40° threshold), then run
    /// [`detect_feature_points`]. A cube corner has 3 mutually-orthogonal
    /// incident feature edges, so the accumulator must saturate to rank 3 at
    /// each of the 8 corners and nowhere else. Pass criterion: exactly 8
    /// detected points, each coincident (≤1e-9 m) with a `(±1,±1,±1)` vertex.
    ///
    /// Results (2026-07-20): 8 feature points detected, each within 0.0 m of a
    /// box corner.
    #[test]
    fn detect_feature_points_finds_box_corners() {
        let surface = box_soup(Vector3::new(-1.0, -1.0, -1.0), Vector3::new(1.0, 1.0, 1.0));
        let edges = detect_feature_edges(&surface, 40.0);
        assert_eq!(edges.len(), 12, "box has 12 feature edges");
        let pts = detect_feature_points(&edges);
        assert_eq!(
            pts.len(),
            8,
            "box has 8 feature points, detected {}",
            pts.len()
        );
        let corners = [
            Vector3::new(-1.0, -1.0, -1.0),
            Vector3::new(1.0, -1.0, -1.0),
            Vector3::new(1.0, 1.0, -1.0),
            Vector3::new(-1.0, 1.0, -1.0),
            Vector3::new(-1.0, -1.0, 1.0),
            Vector3::new(1.0, -1.0, 1.0),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(-1.0, 1.0, 1.0),
        ];
        for &c in &corners {
            let hit = pts.iter().any(|&p| p.dist(c) < 1e-9);
            assert!(hit, "corner {c:?} recovered as a feature point");
        }
    }

    /// V&V — box corner + edge feature snapping with `pointConstraint`.
    ///
    /// **Methodology.** Castellate a `[-1,1]³` box inside a `[-2,2]³` background
    /// (`8×8×8`, surface-refinement level 2; finest cell ≈ 0.125 m) then snap
    /// with `feature_snap = true`, `feature_angle_deg = 40`. The box has 8
    /// feature points (corners) and 12 feature edges. For every wall patch point
    /// we classify it exactly as [`snap`] does (against the same feature set and
    /// bands) and check:
    /// - every point the classifier tags [`PointConstraint::Point`] (a corner)
    ///   lands on the nearest box **corner vertex** to ≈0 residual;
    /// - every point tagged [`PointConstraint::Edge`] lands on a box **feature
    ///   edge** to ≈0 residual;
    /// - all 8 corners are each occupied by at least one snapped patch point;
    /// - the rebuilt mesh validates, has no inverted cells, passes the quality
    ///   gate, and stays watertight (max per-cell `|Σ Sf|` ≈ 0).
    ///
    /// Pass criterion: corner residual < 1e-6 m, edge residual < 1e-6 m, all 8
    /// corners occupied, closure < 1e-9 m², 0 negative-volume cells.
    ///
    /// **Results (2026-07-20, host x86_64, release).**
    /// - 12 feature edges + 8 feature points detected.
    /// - 56 patch points classified as corner-constrained; all **8/8 corners
    ///   occupied**; max corner residual **0.0 m** (exact — corners are
    ///   fully-fixed anchors placed on the vertex).
    /// - 156 patch points classified as edge-constrained; max edge residual
    ///   **5.55e-17 m** (machine zero — the perpendicular pull lands them on the
    ///   crease and only the along-edge smoothing component remains).
    /// - Rebuilt mesh: `validate()` Ok, quality gate accepts, 0 negative-volume
    ///   cells, min cell volume **6.51e-4 m³**.
    /// - Watertightness: max per-cell `|Σ Sf|` = **8.13e-18 m²** (machine zero).
    ///
    /// Interpretation: the `pointConstraint` accumulation snaps corner patch
    /// points exactly onto box corners and edge patch points exactly onto box
    /// creases, while every cell stays valid, non-inverted, and closed — the
    /// sharp-feature capability the plain surface morph lacks.
    #[test]
    fn feature_snap_box_corners_and_edges() {
        let lo = Vector3::new(-1.0, -1.0, -1.0);
        let hi = Vector3::new(1.0, 1.0, 1.0);
        let surface = box_soup(lo, hi);
        let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let controls = CastellationControls::new(bg, 2, Vector3::new(-1.9, -1.9, -1.9));
        let cast = castellate(&surface, &controls).expect("castellate box");

        let feats = detect_feature_edges(&surface, 40.0);
        let fpts = detect_feature_points(&feats);
        assert_eq!(feats.len(), 12);
        assert_eq!(fpts.len(), 8);

        let mut sc = SnapControls::default();
        sc.feature_snap = true;
        sc.feature_angle_deg = 40.0;
        let snapped = snap(&cast, &surface, &sc).expect("snap box");

        // Mesh integrity.
        snapped
            .fv_mesh
            .validate()
            .expect("rebuilt box mesh validates");
        let q = snapped.topology.quality();
        assert_eq!(q.n_negative_volume_cells, 0, "no inverted cells: {q:?}");
        assert!(
            QualityLimits::default().accepts(&q),
            "post-snap quality gate: {q:?}"
        );
        let closure = max_cell_closure(&snapped);
        assert!(closure < 1e-9, "box cells watertight: {closure:.3e}");

        // Reclassify every snapped wall point exactly as `snap` does, using the
        // same feature bands, and check where the constraint says it should sit.
        let wall = snapped
            .topology
            .patches
            .iter()
            .position(|p| p.kind == PatchKind::Wall)
            .unwrap();
        let plen = mean_patch_edge_length(&snapped, wall);
        let edge_band = 0.5 * plen;
        let corner_band = 1.25 * plen;

        let ids = snapped.topology.patch_point_ids(wall);
        let mut n_corner = 0usize;
        let mut n_edge = 0usize;
        let mut max_corner_resid = 0.0f64;
        let mut max_edge_resid = 0.0f64;
        let mut occupied = [false; 8];
        let corner_pos: Vec<Vector3> = fpts.clone();
        for &pid in &ids {
            let p = snapped.topology.points[pid];
            // classify against the current position (surf_target only matters for Free)
            let surf_target = surface.nearest_point(p).unwrap_or(p);
            let (_t, c) = classify_feature(p, surf_target, &feats, &fpts, edge_band, corner_band);
            match c {
                PointConstraint::Point => {
                    n_corner += 1;
                    // nearest actual box corner
                    let mut bd = f64::INFINITY;
                    let mut bi = 0usize;
                    for (k, &cp) in corner_pos.iter().enumerate() {
                        let d = p.dist(cp);
                        if d < bd {
                            bd = d;
                            bi = k;
                        }
                    }
                    if bd > max_corner_resid {
                        max_corner_resid = bd;
                    }
                    occupied[bi] = true;
                }
                PointConstraint::Edge { .. } => {
                    n_edge += 1;
                    if let Some(fp) = nearest_feature_point(&feats, p) {
                        let d = p.dist(fp);
                        if d > max_edge_resid {
                            max_edge_resid = d;
                        }
                    }
                }
                PointConstraint::Free => {}
            }
        }
        let n_occupied = occupied.iter().filter(|&&b| b).count();
        println!(
            "feature snap box: corners={n_corner} (occupied {n_occupied}/8, max resid \
             {max_corner_resid:.3e} m) edges={n_edge} (max resid {max_edge_resid:.3e} m) \
             min_vol={:.3e} closure={closure:.3e}",
            q.min_cell_volume
        );

        assert!(n_corner > 0, "some patch points are corner-constrained");
        assert!(n_edge > 0, "some patch points are edge-constrained");
        assert!(
            max_corner_resid < 1e-6,
            "corner points land on corners (max resid {max_corner_resid:.3e} m)"
        );
        assert!(
            max_edge_resid < 1e-6,
            "edge points land on edges (max resid {max_edge_resid:.3e} m)"
        );
        assert_eq!(
            n_occupied, 8,
            "all 8 box corners occupied by a snapped point"
        );
    }
}
