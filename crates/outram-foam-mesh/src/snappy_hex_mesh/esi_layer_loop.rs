// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream (OpenFOAM-v2506):
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C,
//   `addLayers` (the private overload with the `nLayerIter` loop).

//! gh:#1467 round 9: the layer-addition iteration loop of ESI v2506 `snappyLayerDriver::addLayers`
//! (`for iteration < nLayerIter`): displacement, medial-axis shrink, `truncateDisplacement`,
//! `addPatchCellLayer`, `checkAndUnmark`, `growNoExtrusion`.
//!
//! Scope: one outer iteration (`nOuterIter 1`, so the slice thickness is the overall thickness),
//! serial, no face zones, no baffles, `nSmoothDisplacement 0`. Verification and results:
//! `tests/snappy_hex_mesh_esi_layers.rs`, README "Round 9".

use super::esi_layer_topo::{
    add_patch_cell_layer, avg_point_data_faces, check_and_unmark, count_extrusion, determine_side_patches, edge_global_faces,
    get_layer_cells_faces, setup_layer_info_truncation, truncate_displacement, AddLayerInput, AddedLayers, TruncateReport,
};
use super::esi_layers::{final_layer_thickness_ratio, grow_no_extrusion, ExtrudeState, LayerPatch};
use super::esi_medial::{get_patch_displacement, move_mesh, DisplacementParams, MedialAxis, MoveReport};
use super::esi_merge::MergeMesh;
use super::esi_snap::{mag, mesh_edges, scale, scale_topology, sub, PatchPointScaling, ScaleControls, SnapMesh, V};
use crate::check_mesh::{EsiCheckResult, EsiQualityControls};

/// Inputs of the loop.
#[derive(Debug, Clone)]
pub struct LayerLoopInput<'a> {
    /// The mesh before the layers (after the pre-layer merges), at the original positions.
    pub m: &'a MergeMesh,
    /// The layer patch on `m`.
    pub pp: &'a LayerPatch,
    /// The medial axis fields of `m` (`MedialAxis::update`).
    pub medial: &'a MedialAxis,
    /// `calculateDisplacement` controls.
    pub disp_params: DisplacementParams,
    /// Overall wanted thickness per patch point.
    pub thickness: &'a [f64],
    /// Truncation thickness per patch point.
    pub min_thickness: &'a [f64],
    /// Expansion ratio per patch point.
    pub expansion_ratio: &'a [f64],
    /// `extrudeStatus`, `patchNLayers` after `setPointNumLayers` (`disp` is overwritten).
    pub state: ExtrudeState,
    /// `meshQualityControls` (iterations below `n_relaxed_iter`).
    pub quality: EsiQualityControls,
    /// `meshQualityControls.relaxed` (later iterations).
    pub relaxed_quality: EsiQualityControls,
    /// `nLayerIter`.
    pub n_layer_iter: usize,
    /// `nRelaxedIter`.
    pub n_relaxed_iter: usize,
    /// `nGrow`.
    pub n_grow: usize,
    /// `nBufferCellsNoExtrude`.
    pub n_buffer_cells_no_extrude: i64,
    /// `nRelaxIter` of the mover (`scaleMesh` relax iterations).
    pub n_relax_iter: usize,
    /// `nAllowableErrors` of the mover (`errorReduction`, `nSmoothScale` are ESI's defaults 0.75, 4).
    pub n_allowable_errors: usize,
}

/// What one layer iteration produced.
#[derive(Debug, Clone)]
pub struct LayerIteration {
    /// The shrunk mesh (ESI `writeMesh` "shrunk mesh" time).
    pub shrunk: SnapMesh,
    /// The mover's report.
    pub moved: MoveReport,
    /// What `truncateDisplacement` printed.
    pub truncate: TruncateReport,
    /// Patch faces with `nPatchFaceLayers > 0`.
    pub n_face_layers_positive: usize,
    /// Patch points with `nPatchPointLayers > 0`.
    pub n_point_layers_positive: usize,
    /// The layer mesh.
    pub layers: AddedLayers,
    /// Number of added cells (`cellNLayers > 0`).
    pub n_added_cells: usize,
    /// The quality check of the layer mesh.
    pub check: EsiCheckResult,
    /// Patch faces whose extrusion `checkAndUnmark` removed.
    pub n_changed: usize,
    /// `countExtrusion` after `checkAndUnmark`.
    pub n_extruded_faces: usize,
    /// Per patch face: `cellNLayers`/`faceRealThickness` of the layer mesh.
    pub cell_n_layers: Vec<i64>,
    /// `faceRealThickness` of the layer mesh.
    pub face_real_thickness: Vec<f64>,
    /// `patchNLayers` after the iteration.
    pub patch_n_layers: Vec<i64>,
}

/// `snappyLayerDriver::addLayers` (the iteration loop): runs until `checkAndUnmark` removes nothing
/// or `n_layer_iter` is reached; the last element is the accepted layer mesh.
pub fn add_layers_loop(inp: &LayerLoopInput) -> Vec<LayerIteration> {
    let m = inp.m;
    let pp = inp.pp;
    let n_pp = pp.n_points();
    let old_points = m.mesh.points.clone();
    let me = mesh_edges(&m.mesh);
    let topo = scale_topology(&m.mesh);
    let egf = edge_global_faces(pp);
    let side = determine_side_patches(m, pp);
    let mut st = inp.state.clone();
    let mut out = Vec::new();
    for iteration in 0..inp.n_layer_iter {
        let quality = if iteration < inp.n_relaxed_iter { &inp.quality } else { &inp.relaxed_quality };
        super::esi_medial::sync_patch_displacement(inp.min_thickness, &mut st);
        get_patch_displacement(pp, &old_points, inp.thickness, inp.min_thickness, &mut st);
        let old_patch_pos: Vec<V> = pp.mesh_points.iter().map(|&p| old_points[p]).collect();
        let patch_disp: Vec<V> = st.disp.iter().map(|d| [-d[0], -d[1], -d[2]]).collect();
        let mut mesh = m.mesh.clone();
        let ctl = ScaleControls {
            error_reduction: 0.75,
            n_smooth_scale: 4,
            quality: quality.clone(),
            n_allowable_errors: inp.n_allowable_errors,
            patch_scaling: PatchPointScaling::Scaled,
        };
        let moved = move_mesh(
            &mut mesh,
            &me,
            &topo,
            pp,
            inp.medial,
            inp.min_thickness,
            &inp.disp_params,
            &patch_disp,
            &ctl,
            inp.n_relax_iter,
        );
        for i in 0..n_pp {
            st.disp[i] = sub(old_patch_pos[i], mesh.points[pp.mesh_points[i]]);
        }
        let truncate = truncate_displacement(m, pp, &egf, inp.min_thickness, &[], &mut st);
        let (n_point_layers, n_face_layers) =
            setup_layer_info_truncation(pp, &st.n_layers, &st.status, inp.n_buffer_cells_no_extrude);
        let final_disp: Vec<V> = (0..n_pp)
            .map(|i| scale(final_layer_thickness_ratio(n_point_layers[i], inp.expansion_ratio[i]), st.disp[i]))
            .collect();
        let inv_expansion: Vec<f64> = inp.expansion_ratio.iter().map(|&e| 1.0 / e).collect();
        let shrunk_mesh = MergeMesh { mesh: mesh.clone(), ..m.clone() };
        let layers = add_patch_cell_layer(&AddLayerInput {
            m: &shrunk_mesh,
            pp,
            egf: &egf,
            expansion_ratio: &inv_expansion,
            edge_patch_id: &side,
            n_face_layers: &n_face_layers,
            n_point_layers: &n_point_layers,
            first_layer_disp: &final_disp,
        });
        let mag_disp: Vec<f64> = st.disp.iter().map(|&d| mag(d)).collect();
        let (cell_n_layers, face_real_thickness) =
            get_layer_cells_faces(&layers.mesh.mesh, &layers.layer_faces, &avg_point_data_faces(pp, &mag_disp));
        let n_added_cells = cell_n_layers.iter().filter(|&&c| c > 0).count();
        let (n_changed, check) = check_and_unmark(&layers, quality, pp, &mut st);
        let n_extruded_faces = count_extrusion(pp, &st.status);
        out.push(LayerIteration {
            shrunk: mesh,
            moved,
            truncate,
            n_face_layers_positive: n_face_layers.iter().filter(|&&x| x > 0).count(),
            n_point_layers_positive: n_point_layers.iter().filter(|&&x| x > 0).count(),
            layers,
            n_added_cells,
            check,
            n_changed,
            n_extruded_faces,
            cell_n_layers,
            face_real_thickness,
            patch_n_layers: st.n_layers.clone(),
        });
        if n_changed == 0 {
            break;
        }
        for _ in 0..inp.n_grow {
            grow_no_extrusion(pp, &mut st);
        }
    }
    out
}

/// One row of the table `snappyLayerDriver::printLayerData` prints.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerDataRow {
    /// Patch index.
    pub patch: usize,
    /// Faces of the patch.
    pub n_faces: usize,
    /// `layerParams.numLayers()[patch]`.
    pub target_layers: i64,
    /// `sumNLayers / sumSize`: average number of layers in the cells next to the patch.
    pub avg_layers: f64,
    /// Average real thickness [m].
    pub avg_real: f64,
    /// Average of real / wanted thickness, in percent.
    pub avg_fraction_percent: f64,
}

/// `snappyLayerDriver::printLayerData` (serial) for the mesh of a layer iteration. The wanted
/// thickness of a patch face is `avgPointData(pp, thickness)` of the patch face it descends from
/// (`faceWantedThickness` follows the face map: added layer faces take their master face's value),
/// the real thickness and the layer counts are those of `getLayerCellsFaces`.
pub fn layer_data_table(
    it: &LayerIteration,
    pp: &LayerPatch,
    thickness: &[f64],
    patch_ids: &[usize],
    target_layers: &[i64],
) -> Vec<LayerDataRow> {
    let mesh = &it.layers.mesh;
    let wanted_pp = avg_point_data_faces(pp, thickness);
    let mut wanted = vec![0.0f64; mesh.mesh.faces.len()];
    for (pf, &f_old) in pp.addressing.iter().enumerate() {
        wanted[it.layers.face_map[f_old]] = wanted_pp[pf];
        for &lf in &it.layers.layer_faces[pf] {
            wanted[lf] = wanted_pp[pf];
        }
    }
    let mut rows = Vec::new();
    for &patch in patch_ids {
        let faces: Vec<usize> = (0..mesh.mesh.faces.len()).filter(|&f| mesh.face_patch[f] == patch).collect();
        let size = faces.len();
        let mut sum_n_layers = 0i64;
        let mut sum_real = 0.0f64;
        let mut sum_fraction = 0.0f64;
        for &f in &faces {
            sum_n_layers += it.cell_n_layers[mesh.mesh.owner[f]];
            sum_real += it.face_real_thickness[f];
        }
        for &f in &faces {
            if wanted[f] > 1e-300 {
                sum_fraction += it.face_real_thickness[f] / wanted[f];
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
