// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream (OpenFOAM-v2506):
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C
//   `addLayers` (the outer overload, `for (layeri < nOuterIter)`), `updatePatch`,
//   `printLayerData`, `unmarkExtrusion`;
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/layerParameters/layerParameters.C `layerThickness`.

//! gh:#1467 round 11: the OUTER layer loop of ESI v2506 `snappyLayerDriver::addLayers`
//! (`nOuterIter > 1`). Each outer iteration adds a slice of the layers: `deltaNLayers =
//! (basePatchNLayers + nOuterIter - 1) / nOuterIter` per patch point, with the slice thickness
//! `layerThickness(base n, base thickness, expansion, base n - added - delta, delta)`, by running
//! the inner iteration loop of round 9 ([`add_layers_loop`]) on the CURRENT mesh with a medial axis
//! rebuilt on it. The layer mesh is then applied, the per-point arrays are mapped onto the new
//! layer patch (`updatePatch`: `newToOldPatchPoints`), and `cellNLayers`, `faceRealThickness` and
//! `faceWantedThickness` are carried through the cell and face maps of the topology change.
//!
//! Scope: serial, no face zones or baffles, **closed layer patches** (every patch edge has two patch
//! faces, asserted: ESI's `inflateFaceID` of boundary side faces is not tracked, so the face map of
//! such faces would be a guess). Verification and results: `tests/snappy_hex_mesh_esi_outer.rs`,
//! README "Round 11".

use super::esi_layer_loop::{add_layers_loop, LayerDataRow, LayerIteration, LayerLoopInput};
use super::esi_layer_final::LayerFields;
use super::esi_layer_topo::{added_cells, avg_point_data_faces, determine_side_patches, AddedLayers};
use super::esi_layers::{layer_thickness_slice, ExtrudeMode, ExtrudeState, LayerPatch};
use super::esi_medial::{DisplacementParams, MedialAxis, MedialParams};
use super::esi_merge::MergeMesh;
use super::esi_snap::mesh_edges;
use crate::check_mesh::EsiQualityControls;
use std::collections::HashMap;

/// `VSMALL` of a double.
const VSMALL: f64 = 1e-300;

/// Inputs of the outer loop.
#[derive(Debug, Clone)]
pub struct OuterInput {
    /// The mesh before the layers (after the pre-layer merges).
    pub m: MergeMesh,
    /// The layer patches (indices into the patches of `m`).
    pub patch_ids: Vec<usize>,
    /// `layerParams.numLayers()` per patch index (for the table).
    pub target_layers: Vec<i64>,
    /// `basePatchDisp`, `basePatchNLayers`, `baseExtrudeStatus` after `setPointNumLayers`.
    pub base_state: ExtrudeState,
    /// `baseThickness`: overall wanted thickness per patch point.
    pub base_thickness: Vec<f64>,
    /// `baseMinThickness`.
    pub base_min_thickness: Vec<f64>,
    /// `baseExpansionRatio`.
    pub base_expansion: Vec<f64>,
    /// `nOuterIter`.
    pub n_outer_iter: usize,
    /// Medial axis controls; `n_medial_axis_iter` is replaced by the point count of each outer mesh.
    pub medial: MedialParams,
    /// `calculateDisplacement` controls; `n_medial_axis_iter` likewise replaced.
    pub disp_params: DisplacementParams,
    /// `meshQualityControls`.
    pub quality: EsiQualityControls,
    /// `meshQualityControls.relaxed` merged over the base keys.
    pub relaxed_quality: EsiQualityControls,
    /// `nLayerIter`.
    pub n_layer_iter: usize,
    /// `nRelaxedIter`.
    pub n_relaxed_iter: usize,
    /// `nGrow`.
    pub n_grow: usize,
    /// `nBufferCellsNoExtrude`.
    pub n_buffer_cells_no_extrude: i64,
    /// `nRelaxIter` of the mover.
    pub n_relax_iter: usize,
    /// `nAllowableErrors` of the mover.
    pub n_allowable_errors: usize,
}

/// One outer iteration.
#[derive(Debug, Clone)]
pub struct OuterIteration {
    /// "Layers to add in current iteration": `sum(deltaNLayers)`.
    pub n_to_add: i64,
    /// The inner iterations (round 9's loop).
    pub inner: Vec<LayerIteration>,
    /// "Added in current iteration": `sum(patchNLayers)` after the inner loop.
    pub n_total_added: i64,
    /// `printLayerData` after applying the layer mesh (empty when nothing was added).
    pub table: Vec<LayerDataRow>,
    /// Per patch point `deltaNLayers` after the update (empty when nothing was added).
    pub delta_n_layers_after: Vec<i64>,
}

/// The outcome of the outer loop.
#[derive(Debug, Clone)]
pub struct OuterRun {
    /// The outer iterations that ran (the last one may have added nothing).
    pub outer: Vec<OuterIteration>,
    /// The final mesh: the last applied layer mesh, with the points of the last (empty) inner shrink
    /// when the final outer iteration added nothing (ESI leaves the mesh shrunk at that `break`).
    pub mesh: MergeMesh,
    /// Accumulated `cellNLayers` on `mesh`.
    pub cell_n_layers: Vec<i64>,
    /// Accumulated `faceRealThickness` on `mesh`.
    pub face_real: Vec<f64>,
    /// `faceWantedThickness` on `mesh`.
    pub face_wanted: Vec<f64>,
}

fn n_cells_of_mesh(mesh: &super::esi_snap::SnapMesh) -> usize {
    mesh.owner.iter().chain(mesh.neighbour.iter()).copied().max().map_or(0, |c| c + 1)
}

fn n_cells(m: &MergeMesh) -> usize {
    n_cells_of_mesh(&m.mesh)
}

/// `meshRefinement::updateList(newToOld, nullValue, list)`.
fn update_list<T: Clone>(new_to_old: &[Option<usize>], null_value: T, list: &[T]) -> Vec<T> {
    new_to_old.iter().map(|o| o.map_or_else(|| null_value.clone(), |i| list[i].clone())).collect()
}

/// `mapPolyMesh::pointMap` restricted to what is needed: for every point of the layer mesh the point
/// of the old mesh it came from (old points are not renumbered; an added point's master is the
/// patch point it was extruded from).
fn point_master(layers: &AddedLayers, old_pp: &LayerPatch, n_old_points: usize) -> Vec<usize> {
    let mut master: Vec<usize> = (0..layers.mesh.mesh.points.len()).collect();
    for (j, added) in layers.added_points.iter().enumerate() {
        for &q in added {
            master[q] = old_pp.mesh_points[j];
        }
    }
    debug_assert!(master.iter().take(n_old_points).enumerate().all(|(i, &m)| m == i));
    master
}

/// `snappyLayerDriver::updatePatch`: `newToOldPatchPoints` (`None` for a point not in the old patch).
fn new_to_old_patch_points(old_pp: &LayerPatch, new_pp: &LayerPatch, master: &[usize]) -> Vec<Option<usize>> {
    let base_map: HashMap<usize, usize> = old_pp.mesh_points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    new_pp.mesh_points.iter().map(|&p| base_map.get(&master[p]).copied()).collect()
}

/// `mapPolyMesh::cellMap` and `faceMap` (new to old, `None` for -1) of `addPatchCellLayer` with
/// `addToMesh = true`: old cells and faces map to themselves; an added cell maps to the owner of the
/// patch face it was extruded from (its master cell); a layer face maps to that patch face; the side
/// faces of a closed patch have a master edge, so they map to nothing.
fn cell_and_face_maps(old: &MergeMesh, old_pp: &LayerPatch, layers: &AddedLayers) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let new_mesh = &layers.mesh.mesh;
    let n_new_cells = n_cells_of_mesh(new_mesh);
    let mut cell_map: Vec<Option<usize>> = (0..n_new_cells).map(|c| if c < layers.n_old_cells { Some(c) } else { None }).collect();
    let cells = added_cells(new_mesh, &layers.layer_faces);
    for (pf, added) in cells.iter().enumerate() {
        let master = old.mesh.owner[old_pp.addressing[pf]];
        for &c in added {
            cell_map[c] = Some(master);
        }
    }
    let mut face_map: Vec<Option<usize>> = vec![None; new_mesh.faces.len()];
    for (f_old, &f_new) in layers.face_map.iter().enumerate().take(old.mesh.faces.len()) {
        face_map[f_new] = Some(f_old);
    }
    for (pf, lf) in layers.layer_faces.iter().enumerate() {
        for &f in lf.iter().skip(1) {
            face_map[f] = Some(old_pp.addressing[pf]);
        }
    }
    (cell_map, face_map)
}

/// `printLayerData` for accumulated arrays (the table [`super::esi_layer_loop::layer_data_table`]
/// builds for one iteration is the special case of one outer iteration).
pub fn layer_data_table_accumulated(
    mesh: &MergeMesh,
    cell_n_layers: &[i64],
    face_wanted: &[f64],
    face_real: &[f64],
    patch_ids: &[usize],
    target_layers: &[i64],
) -> Vec<LayerDataRow> {
    let mut rows = Vec::new();
    for &patch in patch_ids {
        let faces: Vec<usize> = (0..mesh.mesh.faces.len()).filter(|&f| mesh.face_patch[f] == patch).collect();
        let size = faces.len();
        let mut sum_n_layers = 0i64;
        let mut sum_real = 0.0f64;
        let mut sum_fraction = 0.0f64;
        for &f in &faces {
            sum_n_layers += cell_n_layers[mesh.mesh.owner[f]];
            sum_real += face_real[f];
        }
        for &f in &faces {
            if face_wanted[f] > VSMALL {
                sum_fraction += face_real[f] / face_wanted[f];
            }
        }
        let n = size as f64;
        rows.push(LayerDataRow {
            patch,
            n_faces: size,
            target_layers: target_layers[patch],
            avg_layers: if size > 0 { sum_n_layers as f64 / n } else { 0.0 },
            avg_real: if size > 0 { sum_real / n } else { 0.0 },
            avg_fraction_percent: if size > 0 { 100.0 * (sum_fraction / n) } else { 0.0 },
        });
    }
    rows
}

/// `writeLayerData` fields for accumulated arrays (see [`super::esi_layer_final::layer_data_fields`]).
pub fn layer_data_fields_accumulated(
    mesh: &MergeMesh,
    cell_n_layers: &[i64],
    face_wanted: &[f64],
    face_real: &[f64],
    patch_ids: &[usize],
) -> LayerFields {
    let mut out = LayerFields {
        n_surface_layers_cells: cell_n_layers.iter().map(|&c| c as f64).collect(),
        n_surface_layers_patches: Vec::new(),
        thickness_patches: Vec::new(),
        thickness_fraction_patches: Vec::new(),
    };
    for &patch in patch_ids {
        let faces: Vec<usize> = (0..mesh.mesh.faces.len()).filter(|&f| mesh.face_patch[f] == patch).collect();
        out.n_surface_layers_patches.push(faces.iter().map(|&f| cell_n_layers[mesh.mesh.owner[f]] as f64).collect());
        out.thickness_patches.push(faces.iter().map(|&f| face_real[f]).collect());
        out.thickness_fraction_patches
            .push(faces.iter().map(|&f| if face_wanted[f] > VSMALL { face_real[f] / face_wanted[f] } else { 0.0 }).collect());
    }
    out
}

/// The slice thickness of one outer iteration (`layerParameters::layerThickness` per patch point).
pub fn slice_thickness(
    base_n_layers: &[i64],
    base_thickness: &[f64],
    base_expansion: &[f64],
    n_added: &[i64],
    delta: &[i64],
) -> Vec<f64> {
    (0..base_thickness.len())
        .map(|i| {
            layer_thickness_slice(base_n_layers[i], base_thickness[i], base_expansion[i], base_n_layers[i] - n_added[i] - delta[i], delta[i])
        })
        .collect()
}

/// `snappyLayerDriver::addLayers` (outer overload).
pub fn add_layers_outer(inp: &OuterInput) -> OuterRun {
    let mut m = inp.m.clone();
    let mut pp = LayerPatch::new(&m, &inp.patch_ids);
    let n_pp0 = pp.n_points();
    assert_eq!(inp.base_thickness.len(), n_pp0, "per-point input sized for the layer patch");
    let mut base = inp.base_state.clone();
    let mut base_thickness = inp.base_thickness.clone();
    let mut base_min = inp.base_min_thickness.clone();
    let mut base_exp = inp.base_expansion.clone();
    let n_outer = inp.n_outer_iter.max(1) as i64;
    let mut delta: Vec<i64> = base.n_layers.iter().map(|&n| (n + n_outer - 1) / n_outer).collect();
    let mut n_added = vec![0i64; n_pp0];
    let mut cell_n_layers = vec![0i64; n_cells(&m)];
    let mut face_real = vec![0.0f64; m.mesh.faces.len()];
    let mut face_wanted = vec![0.0f64; m.mesh.faces.len()];
    for (pf, w) in avg_point_data_faces(&pp, &base_thickness).into_iter().enumerate() {
        face_wanted[pp.addressing[pf]] = w;
    }
    let mut outer = Vec::new();
    for _layeri in 0..inp.n_outer_iter {
        let n_to_add: i64 = delta.iter().sum();
        if n_to_add == 0 {
            break;
        }
        // closed layer patch only (see the module doc)
        assert!(determine_side_patches(&m, &pp).iter().all(|&e| e == -1), "outer iterations need a closed layer patch");
        let slice = slice_thickness(&base.n_layers, &base_thickness, &base_exp, &n_added, &delta);
        let n_points = m.mesh.points.len();
        let me = mesh_edges(&m.mesh);
        let mut mp = inp.medial.clone();
        mp.n_medial_axis_iter = n_points;
        let mut dp = inp.disp_params.clone();
        dp.n_medial_axis_iter = n_points;
        let medial = MedialAxis::update(&m.mesh.points, &me, &pp, &mp);
        let loop_in = LayerLoopInput {
            m: &m,
            pp: &pp,
            medial: &medial,
            disp_params: dp,
            thickness: &slice,
            min_thickness: &base_min,
            expansion_ratio: &base_exp,
            state: ExtrudeState { disp: base.disp.clone(), n_layers: delta.clone(), status: base.status.clone() },
            quality: inp.quality.clone(),
            relaxed_quality: inp.relaxed_quality.clone(),
            n_layer_iter: inp.n_layer_iter,
            n_relaxed_iter: inp.n_relaxed_iter,
            n_grow: inp.n_grow,
            n_buffer_cells_no_extrude: inp.n_buffer_cells_no_extrude,
            n_relax_iter: inp.n_relax_iter,
            n_allowable_errors: inp.n_allowable_errors,
        };
        let inner = add_layers_loop(&loop_in);
        let last = inner.last().expect("at least one inner iteration");
        let patch_n_layers = last.patch_n_layers.clone();
        let n_total_added: i64 = patch_n_layers.iter().sum();
        if n_total_added == 0 {
            // ESI breaks here with the mesh still shrunk by the last inner iteration
            m.mesh.points = last.shrunk.points.clone();
            outer.push(OuterIteration { n_to_add, inner, n_total_added, table: Vec::new(), delta_n_layers_after: Vec::new() });
            break;
        }
        // update the wanted layer statistics
        for i in 0..n_added.len() {
            n_added[i] += patch_n_layers[i];
            if patch_n_layers[i] == 0 {
                base.unmark_point(i);
                base.n_layers[i] = n_added[i];
                delta[i] = 0;
            } else {
                delta[i] = 0.max(delta[i].min(base.n_layers[i] - n_added[i]));
            }
        }
        // apply the layer mesh (the mesh the inner loop's last iteration built)
        let layers = &last.layers;
        let (cell_map, face_map) = cell_and_face_maps(&m, &pp, layers);
        let new_m = layers.mesh.clone();
        let old_cells = cell_n_layers.clone();
        cell_n_layers = update_list(&cell_map, 0i64, &old_cells);
        for (c, v) in cell_n_layers.iter_mut().enumerate() {
            *v += last.cell_n_layers[c];
        }
        let old_real = face_real.clone();
        face_real = update_list(&face_map, 0.0f64, &old_real);
        for (f, v) in face_real.iter_mut().enumerate() {
            *v += last.face_real_thickness[f];
        }
        let old_wanted = face_wanted.clone();
        face_wanted = update_list(&face_map, 0.0f64, &old_wanted);
        let table = layer_data_table_accumulated(&new_m, &cell_n_layers, &face_wanted, &face_real, &inp.patch_ids, &inp.target_layers);
        // updatePatch and the point-based arrays
        let new_pp = LayerPatch::new(&new_m, &inp.patch_ids);
        let master = point_master(layers, &pp, m.mesh.points.len());
        let n2o = new_to_old_patch_points(&pp, &new_pp, &master);
        base.disp = update_list(&n2o, [-1.0; 3], &base.disp);
        base.n_layers = update_list(&n2o, 0i64, &base.n_layers);
        base.status = update_list(&n2o, ExtrudeMode::NoExtrude, &base.status);
        base_thickness = update_list(&n2o, 0.0f64, &base_thickness);
        base_min = update_list(&n2o, 0.0f64, &base_min);
        base_exp = update_list(&n2o, f64::MAX, &base_exp);
        let delta_after = delta.clone();
        delta = update_list(&n2o, 0i64, &delta);
        n_added = update_list(&n2o, 0i64, &n_added);
        outer.push(OuterIteration { n_to_add, inner, n_total_added, table, delta_n_layers_after: delta_after });
        m = new_m;
        pp = new_pp;
    }
    OuterRun { outer, mesh: m, cell_n_layers, face_real, face_wanted }
}
