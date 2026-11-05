// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenFOAM-dev (The OpenFOAM Foundation), commit
// 852527377c7e87e33d51b19ca8974b5db60af28b (2026-11-03):
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C
//     (extrudeMode, unmarkExtrusion, checkManifold, handleNonManifolds,
//      handleFeatureAngle, handleWarpedFaces, setNumLayers, growNoExtrusion,
//      truncateDisplacement, setupLayerInfoTruncation, checkAndUnmark,
//      countExtrusion, addLayers)
//   src/polyTopoChange/polyTopoChange/addPatchCellLayer.C (the point and
//      face layer counts it accepts)
//   etc/caseDicts/mesh/generation/snappyHexMeshDict.cfg (addLayersControls
//      defaults) and meshQualityDict (the quality controls)
// Copyright (C) 2011-2025 OpenFOAM Foundation. Licence: GPL-3.0-or-later,
// conveyed here under GPL-3.0-only.
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

//! snappyHexMesh's **local layer collapse** (gh:#1215):
//! [`LayerQualityStrategy::SnappyCollapse`].
//!
//! Each wall point carries an `extrudeMode` ([`ExtrudeMode`]). The driver
//! [`add_boundary_layers_snappy`] follows `snappyLayerDriver::addLayers`:
//!
//! 1. every point `EXTRUDE` with the requested layers (`setNumLayers`, one
//!    layer patch here), then `handleNonManifolds` (points where the layer
//!    patch is pinched), `handleFeatureAngle` (both points of an edge whose
//!    two layer faces turn by more than `featureAngle`), `handleWarpedFaces`
//!    (only with `relativeSizes`; this crate's sizes are absolute, so it does
//!    not run), `growNoExtrusion` × `nGrow`;
//! 2. up to `nLayerIter` times: the displacement (the per-point cap of
//!    [`crate::layers`] where extruding, zero where not),
//!    `truncateDisplacement` (`minThickness`, then the faces with more than
//!    one change from not-extruded to extruded around them, until none),
//!    `setupLayerInfoTruncation` (per-point layer counts, with the
//!    `nBufferCellsNoExtrude` taper), the layered mesh, OpenFOAM's
//!    `checkMesh` on it ([`faces_failing_mesh_quality`]; the strict
//!    `meshQualityControls` for the first `nRelaxedIter` iterations, the
//!    `relaxed` ones after), and `checkAndUnmark`: every wall face one of
//!    whose layer cells uses a failing face has its points unextruded. When
//!    nothing changed the mesh is kept; otherwise `growNoExtrusion` × `nGrow`
//!    and again. When `nLayerIter` runs out, the last mesh is kept, as
//!    upstream keeps `savedMeshMod`.
//!
//! **Where this differs from upstream, and why.** The mesh is not shrunk by
//! `medialAxisMeshMover` (several cells deep, with the medial-axis
//! thickness limit): only the wall points move, as in the rest of this
//! crate's layerer, and the per-point cap ([`crate::layers`], half the
//! shortest wall edge, upstream cfMesh's rule) stands in for the
//! medial-axis limit. The layer cells are built by
//! [`crate::layers::build_layered_variable`] (see there for how a cell where
//! a layer ends differs from `addPatchCellLayer`'s). `getVertexString` (the
//! "butterfly" faces sharing several consecutive edges) is not ported: the
//! dual's wall faces share single edges. `syncPatchDisplacement` and every
//! parallel exchange are not needed on one process. `minThickness` defaults
//! to zero (disabled): upstream's tutorials set it relative to the cell size
//! (`relativeSizes true`), and with absolute sizes it has no default.
//!
//! Note on `featureAngle`: upstream's `handleFeatureAngle` looks only at
//! edges between *two layer faces*; the rim where a pipe wall meets its
//! inlet is an edge of one layer face, so it does not stop layers there.
//! What collapses layers at such a rim is the quality check.

use crate::checks::{faces_failing_mesh_quality, MeshQualityControls};
use crate::layers::{build_layered_faces, build_layered_variable, layer_setup, LayerOutcome, LayerQualityStrategy, LayerSetup};
use crate::volume_mesh::{cells_faces, from_cell_faces, VolumeMesh};
use petir::mathf::RealMath;
use std::collections::{BTreeMap, BTreeSet};

/// `snappyLayerDriver::extrudeMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeMode {
    /// Extrude normally.
    Extrude,
    /// Do not extrude; the layer collapses to zero thickness here.
    NoExtrude,
    /// Extrude, then remove (upstream sets it for some baffle cases; kept
    /// for the port's completeness, never set here).
    ExtrudeRemove,
}

/// `addLayersControls` and the quality dictionaries the driver uses.
/// Defaults: `etc/caseDicts/mesh/generation/snappyHexMeshDict.cfg` and
/// `meshQualityDict` of OpenFOAM-dev `852527377c7e`.
#[derive(Debug, Clone, PartialEq)]
pub struct SnappyLayerControls {
    /// `featureAngle` [degrees] (cfg: 100).
    pub feature_angle_deg: f64,
    /// `nLayerIter` (cfg: 50).
    pub n_layer_iter: usize,
    /// `nRelaxedIter` (cfg: 20): iterations checked against the strict
    /// controls before the relaxed ones apply.
    pub n_relaxed_iter: usize,
    /// `nGrow` (cfg: 0).
    pub n_grow: usize,
    /// `nBufferCellsNoExtrude` (cfg: 0); negative switches the taper off.
    pub n_buffer_cells_no_extrude: i64,
    /// `minThickness` [m]; 0 disables (see the module docs).
    pub min_thickness: f64,
    /// `meshQualityControls`.
    pub quality: MeshQualityControls,
    /// `meshQualityControls.relaxed.maxNonOrtho` [degrees] (cfg: 75); the
    /// other relaxed values are the strict ones, as in the cfg.
    pub relaxed_max_non_ortho_deg: f64,
    /// How many cells a wall face gets where its points' layer counts
    /// differ (gh:#1241): [`FaceLayerCells::Upstream`] (default) or the
    /// gh:#1215 port's [`FaceLayerCells::MostOfItsPoints`], kept callable
    /// for the differential comparison.
    pub face_cells: FaceLayerCells,
}

/// The cells of a wall face whose points carry different layer counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceLayerCells {
    /// `addPatchCellLayer`'s rule: `nFaceLayers` cells from
    /// `setupLayerInfoTruncation` (1 next to an unextruded point with
    /// `nBufferCellsNoExtrude 0`), the last spanning the rest of each column
    /// ([`crate::layers::build_layered_faces`]).
    Upstream,
    /// The gh:#1215 port: as many cells as the face's most-layered point,
    /// each pinched to zero thickness at a point with fewer layers
    /// ([`crate::layers::build_layered_variable`]). A port defect (gh:#1241:
    /// the pinched stacks fail the tet check and unextrude their neighbours
    /// in turn); kept for the comparison.
    MostOfItsPoints,
}

impl Default for SnappyLayerControls {
    fn default() -> Self {
        SnappyLayerControls {
            feature_angle_deg: 100.0,
            n_layer_iter: 50,
            n_relaxed_iter: 20,
            n_grow: 0,
            n_buffer_cells_no_extrude: 0,
            min_thickness: 0.0,
            quality: MeshQualityControls::default(),
            relaxed_max_non_ortho_deg: 75.0,
            face_cells: FaceLayerCells::Upstream,
        }
    }
}

/// The layer patch's faces and points in patch-local terms
/// (`indirectPrimitivePatch` `localFaces`, `pointFaces`, `edgeFaces`).
struct Patch {
    points: Vec<usize>,
    local: BTreeMap<usize, usize>,
    faces: Vec<Vec<usize>>,
    point_faces: Vec<Vec<usize>>,
    edge_faces: BTreeMap<(usize, usize), Vec<usize>>,
}

impl Patch {
    fn new(mesh: &VolumeMesh, s: &LayerSetup) -> Self {
        let points: Vec<usize> = s.wall_pts.iter().copied().collect();
        let local: BTreeMap<usize, usize> = points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
        let faces: Vec<Vec<usize>> = s.wall_faces.iter().map(|&f| mesh.faces[f].iter().map(|p| local[p]).collect()).collect();
        let mut point_faces = vec![Vec::new(); points.len()];
        let mut edge_faces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (fi, f) in faces.iter().enumerate() {
            let k = f.len();
            for i in 0..k {
                point_faces[f[i]].push(fi);
                let (a, b) = (f[i], f[(i + 1) % k]);
                edge_faces.entry((a.min(b), a.max(b))).or_default().push(fi);
            }
        }
        Patch { points, local, faces, point_faces, edge_faces }
    }
}

/// `unmarkExtrusion(patchPointi, …)`.
fn unmark(p: usize, status: &mut [ExtrudeMode], n_layers: &mut [usize]) -> bool {
    match status[p] {
        ExtrudeMode::Extrude | ExtrudeMode::ExtrudeRemove => {
            status[p] = ExtrudeMode::NoExtrude;
            n_layers[p] = 0;
            true
        }
        ExtrudeMode::NoExtrude => false,
    }
}

/// `unmarkExtrusion(localFace, …)`.
fn unmark_face(f: &[usize], status: &mut [ExtrudeMode], n_layers: &mut [usize]) -> bool {
    let mut any = false;
    for &p in f {
        any |= unmark(p, status, n_layers);
    }
    any
}

/// `checkManifold` + `handleNonManifolds` (local checks only): unextrude
/// points where the patch is pinched (a point whose faces form more than one
/// edge-connected fan) and both points of an edge with more than two faces.
fn handle_non_manifolds(pp: &Patch, status: &mut [ExtrudeMode], n_layers: &mut [usize]) -> usize {
    let mut non_manifold: BTreeSet<usize> = BTreeSet::new();
    for (&(a, b), fs) in &pp.edge_faces {
        if fs.len() > 2 {
            non_manifold.insert(a);
            non_manifold.insert(b);
        }
    }
    for (p, fs) in pp.point_faces.iter().enumerate() {
        // Fans: faces around p joined through an edge (p, q).
        let mut comp: Vec<usize> = (0..fs.len()).collect();
        fn root(c: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while c[i] != i {
                c[i] = c[c[i]];
                i = c[i];
            }
            i
        }
        for i in 0..fs.len() {
            for j in i + 1..fs.len() {
                let (fi, fj) = (&pp.faces[fs[i]], &pp.faces[fs[j]]);
                let share = fi.iter().any(|&q| q != p && fj.contains(&q) && {
                    let e = (p.min(q), p.max(q));
                    pp.edge_faces.get(&e).is_some_and(|ef| ef.contains(&fs[i]) && ef.contains(&fs[j]))
                });
                if share {
                    let (ri, rj) = (root(&mut comp, i), root(&mut comp, j));
                    comp[ri] = rj;
                }
            }
        }
        let roots: BTreeSet<usize> = (0..fs.len()).map(|i| root(&mut comp, i)).collect();
        if roots.len() > 1 {
            non_manifold.insert(p);
        }
    }
    let mut n = 0;
    for p in non_manifold {
        n += usize::from(unmark(p, status, n_layers));
    }
    n
}

/// `handleFeatureAngle(minCos)`: for every patch edge with two faces whose
/// normals' dot product is below `cos(featureAngle)`, unextrude both points.
fn handle_feature_angle(mesh: &VolumeMesh, s: &LayerSetup, pp: &Patch, min_cos: f64, status: &mut [ExtrudeMode], n_layers: &mut [usize]) -> usize {
    if min_cos >= 1.0 - 1e-15 {
        return 0;
    }
    let normals: Vec<_> = s.wall_faces.iter().map(|&f| mesh.face_area_vector(f).normalize()).collect();
    let mut n = 0;
    for (&(a, b), fs) in &pp.edge_faces {
        if fs.len() == 2 && normals[fs[0]].dot(normals[fs[1]]) < min_cos {
            unmark(a, status, n_layers);
            unmark(b, status, n_layers);
            n += 1;
        }
    }
    n
}

/// `growNoExtrusion`: every point of a face with a not-extruded point is
/// unextruded.
fn grow_no_extrusion(pp: &Patch, status: &mut Vec<ExtrudeMode>, n_layers: &mut [usize]) -> usize {
    let mut grown = status.clone();
    let mut n = 0;
    for f in &pp.faces {
        if f.iter().any(|&p| status[p] == ExtrudeMode::NoExtrude) {
            for &p in f {
                if status[p] == ExtrudeMode::Extrude && grown[p] != ExtrudeMode::NoExtrude {
                    grown[p] = ExtrudeMode::NoExtrude;
                    n += 1;
                }
            }
        }
    }
    *status = grown;
    for (p, st) in status.iter().enumerate() {
        if *st == ExtrudeMode::NoExtrude {
            n_layers[p] = 0;
        }
    }
    n
}

/// `truncateDisplacement` without the illegal-face set and the butterfly
/// strings (module docs): `minThickness`, then the pinched faces (more than
/// one transition from not-extruded to extruded around the face) until none.
fn truncate_displacement(pp: &Patch, thickness: &[f64], min_thickness: f64, status: &mut [ExtrudeMode], n_layers: &mut [usize]) -> usize {
    let mut n = 0;
    for p in 0..status.len() {
        if thickness[p] < min_thickness {
            n += usize::from(unmark(p, status, n_layers));
        } else if status[p] == ExtrudeMode::NoExtrude {
            n_layers[p] = 0;
        }
    }
    loop {
        let mut n_pinched = 0;
        for f in &pp.faces {
            let mut n_trans = 0;
            let mut prev = status[*f.last().unwrap()];
            for &p in f {
                let m = status[p];
                if prev == ExtrudeMode::NoExtrude && m != ExtrudeMode::NoExtrude {
                    n_trans += 1;
                }
                prev = m;
            }
            if n_trans > 1 && unmark_face(f, status, n_layers) {
                n_pinched += 1;
                n += 1;
            }
        }
        if n_pinched == 0 {
            break;
        }
    }
    n
}

/// `setupLayerInfoTruncation`: the per-point layer counts.
fn setup_layer_info_truncation(pp: &Patch, n_layers: &[usize], status: &[ExtrudeMode], n_buffer: i64) -> (Vec<usize>, Vec<usize>) {
    let nf = pp.faces.len();
    let mut face_layers: Vec<i64> = vec![-1; nf];
    let mut point_layers = vec![0usize; pp.points.len()];
    if n_buffer < 0 {
        for (fi, f) in pp.faces.iter().enumerate() {
            if let Some(&p) = f.iter().find(|&&p| n_layers[p] > 0) {
                face_layers[fi] = n_layers[p] as i64;
            }
        }
        for v in face_layers.iter_mut() {
            if *v == -1 {
                *v = 0;
            }
        }
        point_layers.copy_from_slice(n_layers);
        return (point_layers, face_layers.iter().map(|&v| v as usize).collect());
    }
    let mut max_level = vec![0usize; nf];
    for (fi, f) in pp.faces.iter().enumerate() {
        let no_extrude = f.iter().any(|&p| status[p] == ExtrudeMode::NoExtrude);
        let m = f.iter().map(|&p| n_layers[p]).max().unwrap_or(0);
        if m > 0 {
            if no_extrude {
                face_layers[fi] = 1;
            }
            max_level[fi] = m;
        }
    }
    let n_levels = n_layers.iter().copied().max().unwrap_or(0);
    for ilevel in 1..n_levels {
        let n_buf = if ilevel == 1 { n_buffer - 1 } else { n_buffer };
        let mut ibuffer = 0i64;
        while ibuffer < n_buf + 1 {
            let mut temp = face_layers.clone();
            let found: Vec<bool> = pp.point_faces.iter().map(|fs| fs.iter().any(|&f| face_layers[f] != -1 && max_level[f] > 0)).collect();
            for (p, fs) in pp.point_faces.iter().enumerate() {
                if found[p] {
                    for &f in fs {
                        if face_layers[f] == -1 && max_level[f] > 0 && ilevel < max_level[f] {
                            temp[f] = ilevel as i64;
                        }
                    }
                }
            }
            face_layers = temp;
            ibuffer += 1;
        }
    }
    for (fi, v) in face_layers.iter_mut().enumerate() {
        if *v == -1 {
            *v = max_level[fi] as i64;
        }
    }
    for (p, fs) in pp.point_faces.iter().enumerate() {
        if status[p] != ExtrudeMode::NoExtrude {
            for &f in fs {
                point_layers[p] = point_layers[p].max(face_layers[f] as usize);
            }
        }
    }
    (point_layers, face_layers.iter().map(|&v| v as usize).collect())
}

/// [`LayerQualityStrategy::SnappyCollapse`]: grow `n_layers` layers on
/// `layer_patches` and collapse them locally where the layered mesh fails
/// snappyHexMesh's quality controls (module docs). Returns the mesh and what
/// happened; the input rebuilt with no layer when the layer cannot be set
/// up.
pub fn add_boundary_layers_snappy(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
    controls: &SnappyLayerControls,
) -> (VolumeMesh, LayerOutcome) {
    let strategy = LayerQualityStrategy::SnappyCollapse;
    let Some(s) = layer_setup(mesh, layer_patches, n_layers, first_thickness, expansion) else {
        return (from_cell_faces(mesh.points.clone(), &cells_faces(mesh)), LayerOutcome::none(strategy, n_layers));
    };
    let pp = Patch::new(mesh, &s);
    let np = pp.points.len();
    let mut status = vec![ExtrudeMode::Extrude; np];
    let mut pnl = vec![n_layers; np]; // setNumLayers: one layer patch
    let mut notes = Vec::new();
    let n_nm = handle_non_manifolds(&pp, &mut status, &mut pnl);
    let n_feat = handle_feature_angle(mesh, &s, &pp, controls.feature_angle_deg.to_radians().r_cos(), &mut status, &mut pnl);
    for _ in 0..controls.n_grow {
        grow_no_extrusion(&pp, &mut status, &mut pnl);
    }
    notes.push(format!("handleNonManifolds: {n_nm} points; handleFeatureAngle: {n_feat} edges"));

    let mut last: Option<(VolumeMesh, BTreeMap<usize, usize>, BTreeMap<usize, f64>)> = None;
    let mut iterations = 0;
    let mut remaining = 0;
    let mut converged = false;
    for iteration in 0..controls.n_layer_iter {
        iterations = iteration + 1;
        let mut quality = controls.quality.clone();
        if iteration >= controls.n_relaxed_iter {
            quality.max_non_ortho_deg = controls.relaxed_max_non_ortho_deg;
        }
        let thickness: Vec<f64> = (0..np).map(|p| if status[p] == ExtrudeMode::NoExtrude { 0.0 } else { s.cap[&pp.points[p]] }).collect();
        let n_truncated = truncate_displacement(&pp, &thickness, controls.min_thickness, &mut status, &mut pnl);
        let (point_layers, face_layers) = setup_layer_info_truncation(&pp, &pnl, &status, controls.n_buffer_cells_no_extrude);
        let counts: BTreeMap<usize, usize> = pp.points.iter().enumerate().map(|(i, &p)| (p, point_layers[i])).collect();
        let totals: BTreeMap<usize, f64> = pp.points.iter().map(|&p| (p, s.cap[&p])).collect();
        let layered = match controls.face_cells {
            FaceLayerCells::Upstream => build_layered_faces(mesh, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &totals, &counts, &face_layers),
            FaceLayerCells::MostOfItsPoints => build_layered_variable(mesh, &s.wall_faces, &s.wall_pts, &s.inward, &s.d, &totals, &counts),
        };
        let wrong = faces_failing_mesh_quality(&layered, &quality);
        // checkAndUnmark: cells added for each wall face, in order.
        let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); layered.n_cells];
        for f in 0..layered.face_count() {
            cell_faces[layered.owner[f]].push(f);
            if let Some(n) = layered.neighbour[f] {
                cell_faces[n].push(f);
            }
        }
        let mut next_cell = mesh.n_cells;
        let mut n_changed = 0;
        let mut n_wrong_layer = 0;
        let mut n_wrong_pinched = 0;
        for (fi, f) in pp.faces.iter().enumerate() {
            let m_f = match controls.face_cells {
                FaceLayerCells::Upstream => face_layers[fi],
                FaceLayerCells::MostOfItsPoints => f.iter().map(|&p| point_layers[p]).max().unwrap_or(0),
            };
            let pinched = f.iter().any(|&p| point_layers[p] < m_f);
            let cells = next_cell..next_cell + m_f;
            next_cell += m_f;
            let uses_wrong = cells.clone().any(|c| cell_faces[c].iter().any(|&g| wrong[g]));
            if uses_wrong {
                n_wrong_layer += 1;
                n_wrong_pinched += usize::from(pinched);
                if unmark_face(&pp.faces[fi], &mut status, &mut pnl) {
                    n_changed += 1;
                }
            }
        }
        remaining = n_wrong_layer;
        // Per-iteration trace (gh:#1241): what failed and where.
        notes.push(format!(
            "iteration {iterations}: {n_truncated} faces unextruded by truncateDisplacement first; {} failing faces; {n_wrong_layer} wall faces use one ({n_wrong_pinched} of them with a point of fewer layers), {n_changed} unmarked; {} of {np} points extruded",
            wrong.iter().filter(|&&b| b).count(),
            status.iter().filter(|&&m| m != ExtrudeMode::NoExtrude).count()
        ));
        last = Some((layered, counts, totals));
        if n_changed == 0 {
            converged = true;
            break;
        }
        for _ in 0..controls.n_grow {
            grow_no_extrusion(&pp, &mut status, &mut pnl);
        }
    }
    let (layered, counts, totals) = last.expect("nLayerIter >= 1");
    let mut outcome = LayerOutcome::from_columns(strategy, mesh, &s, &counts, &totals, iterations);
    outcome.remaining_bad_faces = remaining;
    let n_collapsed = status.iter().filter(|&&m| m == ExtrudeMode::NoExtrude).count();
    notes.push(format!(
        "{} after {iterations} layer iteration(s): {n_collapsed} of {np} wall points not extruded; {remaining} wall faces whose layer cells still use a failing face",
        if converged { "converged" } else { "nLayerIter exhausted" }
    ));
    outcome.notes = notes;
    let _ = pp.local.len();
    (layered, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `n` unit quads in z = 0 (2 × (n+1) points), as a patch.
    fn strip(n: usize) -> Patch {
        let faces: Vec<Vec<usize>> = (0..n).map(|i| vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1]).collect();
        let np = 2 * (n + 1);
        let mut point_faces = vec![Vec::new(); np];
        let mut edge_faces: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (fi, f) in faces.iter().enumerate() {
            for i in 0..4 {
                point_faces[f[i]].push(fi);
                let (a, b) = (f[i], f[(i + 1) % 4]);
                edge_faces.entry((a.min(b), a.max(b))).or_default().push(fi);
            }
        }
        Patch { points: (0..np).collect(), local: BTreeMap::new(), faces, point_faces, edge_faces }
    }

    /// V&V (gh:#1215) — `setupLayerInfoTruncation` against hand values.
    /// A strip of six quads (face i on points 2i, 2i+1, 2i+2, 2i+3), 3
    /// layers wanted everywhere, points 0 and 1 not extruded.
    ///
    /// Hand trace written before the first run: with `nBufferCellsNoExtrude
    /// 0`, face 0 is the only seed and "no ring grows", so every extruded
    /// point takes 3. **Missed:** upstream's loop bounds (`nBuffer =
    /// nBufferCellsNoExtrude − 1` at level 1, `nBufferCellsNoExtrude` above)
    /// give level 1 no pass but level 2 one pass, so face 1 gets 2 and
    /// points 2, 3 read 2 (re-read in `snappyLayerDriver.C`, the code follows
    /// it). With a negative value the taper is off and every extruded point
    /// takes 3. With 2, level 1 grows two rings (faces 1, 2 get 1) and level 2
    /// three (faces 3, 4, 5 get 2): points 0, 0 | 1 × 4 | 2 × 8.
    #[test]
    fn layer_info_truncation_tapers_as_upstream() {
        let pp = strip(6);
        let mut status = vec![ExtrudeMode::Extrude; 14];
        let mut pnl = vec![3usize; 14];
        unmark(0, &mut status, &mut pnl);
        unmark(1, &mut status, &mut pnl);
        let pl = setup_layer_info_truncation(&pp, &pnl, &status, 0).0;
        assert_eq!(pl, vec![0, 0, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3]);
        let pl = setup_layer_info_truncation(&pp, &pnl, &status, -1).0;
        assert_eq!(pl, vec![0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3]);
        // nBuffer 2: level 1 grows 2 rings (faces 1, 2 -> 1), level 2 grows
        // 3 rings (faces 3, 4, 5 -> 2); face 0 keeps its seed 1.
        let pl = setup_layer_info_truncation(&pp, &pnl, &status, 2).0;
        assert_eq!(pl, vec![0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2]);
    }

    /// V&V (gh:#1215) — `growNoExtrusion` and the pinched-face rule of
    /// `truncateDisplacement`. On the strip, unextruding point 0 then one
    /// `growNoExtrusion` unextrudes the other points of face 0 (1, 2, 3).
    /// A single quad with its points not-extruded / extruded alternately
    /// (two transitions) is pinched and fully unextruded.
    #[test]
    fn grow_and_pinch_follow_upstream() {
        let pp = strip(3);
        let mut status = vec![ExtrudeMode::Extrude; 8];
        let mut pnl = vec![2usize; 8];
        unmark(0, &mut status, &mut pnl);
        assert_eq!(grow_no_extrusion(&pp, &mut status, &mut pnl), 3);
        assert_eq!(&pnl[..4], &[0, 0, 0, 0]);
        assert!(status[4..].iter().all(|&m| m == ExtrudeMode::Extrude));
        let pp = strip(1);
        let mut status = vec![ExtrudeMode::NoExtrude, ExtrudeMode::Extrude, ExtrudeMode::Extrude, ExtrudeMode::NoExtrude];
        // ring order 0, 2, 3, 1: No, Ex, No, Ex -> two transitions
        let mut pnl = vec![0, 2, 2, 0];
        let n = truncate_displacement(&pp, &[0.0, 1.0, 1.0, 0.0], 0.0, &mut status, &mut pnl);
        assert_eq!(n, 1);
        assert!(status.iter().all(|&m| m == ExtrudeMode::NoExtrude));
    }
}
