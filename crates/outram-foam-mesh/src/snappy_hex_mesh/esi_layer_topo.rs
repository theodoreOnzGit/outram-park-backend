// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream (OpenFOAM-v2506):
//   src/mesh/snappyHexMesh/snappyHexMeshDriver/snappyLayerDriver.C
//     (truncateDisplacement, getVertexString, sameEdgeNeighbour, setupLayerInfoTruncation,
//      checkAndUnmark, countExtrusion, getLayerCellsFaces),
//   src/dynamicMesh/polyTopoChange/polyTopoChange/addPatchCellLayer.C
//     (setRefinement, getEdgeString, addSideFace, findDuplicatedPoints, addedCells),
//   src/meshTools/polyTopoChange/polyTopoChange.C (makeCells, getFaceOrder, compact).

//! gh:#1467 round 9: the layer topology of ESI v2506 `snappyHexMesh`: `truncateDisplacement`,
//! `setupLayerInfoTruncation`, `addPatchCellLayer::setRefinement` with `polyTopoChange`'s numbering,
//! and `checkAndUnmark`.
//!
//! Scope (the ESI call `addPatchCellLayer addLayer(mesh)` of `snappyLayerDriver::addLayers`):
//! `addToMesh = true`, `extrude = true` (the constructor defaults), serial, **no face zones** (zone and `inflateFaceID` arguments
//! only feed the maps `mapPolyMesh` and are not modelled: they do not change a face, an owner or a
//! number), `ppFlip` all false (the driver passes `bitSet(pp.size())`). A flip map with a set bit is
//! rejected. `polyTopoChange` is modelled for the changes `addPatchCellLayer` makes (added points, cells
//! and faces, modified faces and points) with `orderCells = orderPoints = false`, no removals.
//!
//! Verification: `tests/snappy_hex_mesh_esi_layers.rs` against ESI's `86/polyMesh` and later times;
//! methodology, predictions and results in `verification_and_validation/snappy_hex_mesh_esi_v2506/README.md`,
//! "Round 9".

use std::collections::HashMap;

use super::esi_layers::{ExtrudeMode, ExtrudeState, LayerPatch};
use super::esi_medial::sync_patch_displacement;
use super::esi_merge::MergeMesh;
use super::esi_snap::{add, mag, scale, sub, SnapMesh, V};
use crate::check_mesh::{esi_check_mesh, esi_geometry, EsiCheckResult, EsiQualityControls};

// ---------------------------------------------------------------------------------
// truncateDisplacement
// ---------------------------------------------------------------------------------

/// `edgeGlobalFaces` in serial: the mesh faces (global index = mesh index) of the patch faces
/// using each patch edge (`addPatchCellLayer::globalEdgeFaces`, empty flip map).
pub fn edge_global_faces(pp: &LayerPatch) -> Vec<Vec<usize>> {
    pp.edge_faces.iter().map(|ef| ef.iter().map(|&f| pp.addressing[f]).collect()).collect()
}

fn same_edge_pair(a: (usize, usize), b: (usize, usize)) -> bool {
    (a.0 == b.0 && a.1 == b.1) || (a.0 == b.1 && a.1 == b.0)
}

/// `snappyLayerDriver::sameEdgeNeighbour`.
fn same_edge_neighbour_driver(egf: &[Vec<usize>], my_glob: usize, nbr_glob: usize, edgei: usize) -> bool {
    let e = &egf[edgei];
    e.len() == 2 && same_edge_pair((my_glob, nbr_glob), (e[0], e[1]))
}

/// `snappyLayerDriver::getVertexString`: the vertices of the string of face `facei` edges that
/// share the same neighbour face across `edgei`, plus one vertex before and after.
pub fn get_vertex_string(
    pp: &LayerPatch,
    egf: &[Vec<usize>],
    facei: usize,
    edgei: usize,
    my_glob: usize,
    nbr_glob: usize,
) -> Vec<usize> {
    let f_edges = &pp.face_edges[facei];
    let n = f_edges.len();
    let fp = f_edges.iter().position(|&e| e == edgei).expect("getVertexString: edge not on face");
    let mut start_fp = fp;
    for _ in 0..n {
        let prev = (start_fp + n - 1) % n;
        if !same_edge_neighbour_driver(egf, my_glob, nbr_glob, f_edges[prev]) {
            break;
        }
        start_fp = prev;
    }
    let mut end_fp = fp;
    for _ in 0..n {
        let next = (end_fp + 1) % n;
        if !same_edge_neighbour_driver(egf, my_glob, nbr_glob, f_edges[next]) {
            break;
        }
        end_fp = next;
    }
    let f = &pp.local_faces[facei];
    let mut vertices = Vec::new();
    let mut p = start_fp;
    while p != end_fp {
        vertices.push(f[p]);
        p = (p + 1) % n;
    }
    vertices.push(f[p]);
    p = (p + 1) % n;
    vertices.push(f[p]);
    vertices
}

/// What `truncateDisplacement` printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncateReport {
    /// The return value: points unmarked in total.
    pub n_changed: usize,
    /// Per pass of the `while (true)` loop: (faces unextruded for non-consecutive vertices,
    /// for stringed edges with inconsistent extrusion).
    pub passes: Vec<(usize, usize)>,
}

/// `snappyLayerDriver::truncateDisplacement` (serial). `illegal_patch_faces` are boundary mesh
/// faces (`faceSet illegalPatchFaces`; the driver passes an empty one).
pub fn truncate_displacement(
    m: &MergeMesh,
    pp: &LayerPatch,
    egf: &[Vec<usize>],
    min_thickness: &[f64],
    illegal_patch_faces: &[usize],
    st: &mut ExtrudeState,
) -> TruncateReport {
    let mut n_changed = 0usize;
    let point_map: HashMap<usize, usize> = pp.mesh_points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    for &facei in illegal_patch_faces {
        assert!(facei >= m.mesh.n_internal_faces(), "truncateDisplacement: faceset contains internal face {facei}");
        for &p in &m.mesh.faces[facei] {
            if let Some(&pi) = point_map.get(&p) {
                if st.status[pi] != ExtrudeMode::NoExtrude {
                    st.unmark_point(pi);
                    n_changed += 1;
                }
            }
        }
    }
    for pi in 0..st.disp.len() {
        if mag(st.disp[pi]) < min_thickness[pi] {
            if st.unmark_point(pi) {
                n_changed += 1;
            }
        } else if st.status[pi] == ExtrudeMode::NoExtrude {
            st.disp[pi] = [0.0; 3];
            st.n_layers[pi] = 0;
        }
    }
    let mut passes = Vec::new();
    loop {
        sync_patch_displacement(min_thickness, st);
        let mut n_pinched = 0usize;
        for local_f in &pp.local_faces {
            let mut n_trans = 0;
            let mut prev_mode = st.status[*local_f.last().unwrap()];
            for &p in local_f {
                let fp_mode = st.status[p];
                if prev_mode == ExtrudeMode::NoExtrude && fp_mode != ExtrudeMode::NoExtrude {
                    n_trans += 1;
                }
                prev_mode = fp_mode;
            }
            if n_trans > 1 && st.unmark_face(local_f) {
                n_pinched += 1;
                n_changed += 1;
            }
        }
        let mut n_butterfly = 0usize;
        for edgei in 0..pp.edges.len() {
            let glob_faces = &egf[edgei];
            if glob_faces.len() == 2 {
                let my_facei = pp.edge_faces[edgei][0];
                let my_glob = pp.addressing[my_facei];
                let nbr_glob = if glob_faces[0] != my_glob { glob_faces[0] } else { glob_faces[1] };
                let stringed = get_vertex_string(pp, egf, my_facei, edgei, my_glob, nbr_glob);
                if st.status[stringed[0]] != ExtrudeMode::NoExtrude
                    || st.status[*stringed.last().unwrap()] != ExtrudeMode::NoExtrude
                {
                    let pinch = (1..stringed.len() - 1).any(|i| st.status[stringed[i]] == ExtrudeMode::NoExtrude);
                    if pinch {
                        for &v in &stringed {
                            if st.unmark_point(v) {
                                n_butterfly += 1;
                                n_changed += 1;
                            }
                        }
                    }
                }
            }
        }
        passes.push((n_pinched, n_butterfly));
        if n_pinched + n_butterfly == 0 {
            break;
        }
    }
    TruncateReport { n_changed, passes }
}

// ---------------------------------------------------------------------------------
// setupLayerInfoTruncation
// ---------------------------------------------------------------------------------

/// `snappyLayerDriver::setupLayerInfoTruncation` (serial, so the `syncTools` calls are
/// identities): returns `(nPatchPointLayers, nPatchFaceLayers)`, both starting at -1.
pub fn setup_layer_info_truncation(
    pp: &LayerPatch,
    patch_n_layers: &[i64],
    st_status: &[ExtrudeMode],
    n_buffer_cells_no_extrude: i64,
) -> (Vec<i64>, Vec<i64>) {
    let n_points = pp.n_points();
    let n_faces = pp.local_faces.len();
    let mut n_point_layers = vec![-1i64; n_points];
    let mut n_face_layers = vec![-1i64; n_faces];
    if n_buffer_cells_no_extrude < 0 {
        for (pf, f) in pp.local_faces.iter().enumerate() {
            for &p in f {
                let n = patch_n_layers[p];
                if n > 0 {
                    n_face_layers[pf] = if n_face_layers[pf] == -1 { n } else { n_face_layers[pf].min(n) };
                }
            }
        }
        n_point_layers = patch_n_layers.to_vec();
        for x in n_face_layers.iter_mut() {
            if *x == -1 {
                *x = 0;
            }
        }
    } else {
        let mut max_level = vec![0i64; n_faces];
        for (pf, f) in pp.local_faces.iter().enumerate() {
            let mut no_extrude = false;
            let mut m_level = 0i64;
            for &p in f {
                if st_status[p] == ExtrudeMode::NoExtrude {
                    no_extrude = true;
                }
                m_level = m_level.max(patch_n_layers[p]);
            }
            if m_level > 0 {
                if no_extrude {
                    n_face_layers[pf] = 1;
                    max_level[pf] = m_level;
                } else {
                    max_level[pf] = m_level;
                }
            }
        }
        let n_levels = patch_n_layers.iter().copied().max().unwrap_or(0);
        for ilevel in 1..n_levels {
            let n_buffer = if ilevel == 1 { n_buffer_cells_no_extrude - 1 } else { n_buffer_cells_no_extrude };
            for _ in 0..(n_buffer + 1) {
                let mut temp_counter = n_face_layers.clone();
                let mut found_neighbour = vec![false; n_points];
                for pi in 0..n_points {
                    for &fi in &pp.point_faces[pi] {
                        if n_face_layers[fi] != -1 && max_level[fi] > 0 {
                            found_neighbour[pi] = true;
                            break;
                        }
                    }
                }
                for pi in 0..n_points {
                    if found_neighbour[pi] {
                        for &fi in &pp.point_faces[pi] {
                            if n_face_layers[fi] == -1 && max_level[fi] > 0 && ilevel < max_level[fi] {
                                temp_counter[fi] = ilevel;
                            }
                        }
                    }
                }
                n_face_layers = temp_counter;
            }
        }
        for pf in 0..n_faces {
            if n_face_layers[pf] == -1 {
                n_face_layers[pf] = max_level[pf];
            }
        }
        for pi in 0..n_points {
            if st_status[pi] != ExtrudeMode::NoExtrude {
                for &fi in &pp.point_faces[pi] {
                    n_point_layers[pi] = n_point_layers[pi].max(n_face_layers[fi]);
                }
            } else {
                n_point_layers[pi] = 0;
            }
        }
    }
    (n_point_layers, n_face_layers)
}

/// `snappyLayerDriver::countExtrusion`: patch faces with at least one extruded point.
pub fn count_extrusion(pp: &LayerPatch, status: &[ExtrudeMode]) -> usize {
    pp.local_faces.iter().filter(|f| f.iter().any(|&p| status[p] != ExtrudeMode::NoExtrude)).count()
}

// ---------------------------------------------------------------------------------
// The polyTopoChange accumulator and its numbering
// ---------------------------------------------------------------------------------

/// The state of a `polyTopoChange` that only adds points, cells and faces and modifies faces and
/// points: labels are the `polyTopoChange` labels (old ones first, then added in order of calls).
#[derive(Debug, Clone)]
struct TopoBuilder {
    points: Vec<V>,
    faces: Vec<Vec<usize>>,
    owner: Vec<usize>,
    /// -1 for a boundary face.
    neighbour: Vec<i64>,
    /// Patch of a boundary face, -1 for an internal face.
    region: Vec<i64>,
    n_cells: usize,
}

impl TopoBuilder {
    fn from_mesh(m: &MergeMesh, n_cells: usize) -> Self {
        let n_int = m.mesh.n_internal_faces();
        let mut neighbour: Vec<i64> = m.mesh.neighbour.iter().map(|&x| x as i64).collect();
        neighbour.resize(m.mesh.faces.len(), -1);
        let region: Vec<i64> =
            (0..m.mesh.faces.len()).map(|f| if f < n_int { -1 } else { m.face_patch[f] as i64 }).collect();
        Self {
            points: m.mesh.points.clone(),
            faces: m.mesh.faces.clone(),
            owner: m.mesh.owner.clone(),
            neighbour,
            region,
            n_cells,
        }
    }

    fn add_point(&mut self, p: V) -> usize {
        self.points.push(p);
        self.points.len() - 1
    }

    fn add_cell(&mut self) -> usize {
        self.n_cells += 1;
        self.n_cells - 1
    }

    fn add_face(&mut self, ring: Vec<usize>, own: usize, nei: i64, patch: i64) -> usize {
        self.faces.push(ring);
        self.owner.push(own);
        self.neighbour.push(nei);
        self.region.push(patch);
        self.faces.len() - 1
    }

    fn modify_face(&mut self, facei: usize, ring: Vec<usize>, own: usize, nei: i64, patch: i64) {
        self.faces[facei] = ring;
        self.owner[facei] = own;
        self.neighbour[facei] = nei;
        self.region[facei] = patch;
    }

    /// `polyTopoChange::makeCells` + `getFaceOrder` + `compact` (nothing removed, nothing
    /// reordered but the faces): returns the mesh and the new label of every face.
    fn make_mesh(&self, n_patches: usize, constraint: &[bool]) -> (MergeMesh, Vec<usize>) {
        let n_faces = self.faces.len();
        // cellFaces: owned faces ascending, then faces the cell neighbours, ascending.
        let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); self.n_cells];
        for f in 0..n_faces {
            cell_faces[self.owner[f]].push(f);
        }
        for f in 0..n_faces {
            if self.neighbour[f] >= 0 {
                cell_faces[self.neighbour[f] as usize].push(f);
            }
        }
        let mut old_to_new = vec![usize::MAX; n_faces];
        let mut new_face = 0usize;
        for celli in 0..self.n_cells {
            let mut nbr: Vec<(i64, usize)> = Vec::new();
            for (i, &f) in cell_faces[celli].iter().enumerate() {
                let mut nbr_cell = self.neighbour[f];
                let key = if nbr_cell != -1 {
                    if nbr_cell == celli as i64 {
                        nbr_cell = self.owner[f] as i64;
                    }
                    if (celli as i64) < nbr_cell {
                        nbr_cell
                    } else {
                        -1
                    }
                } else {
                    -1
                };
                nbr.push((key, i));
            }
            // sortedOrder is a stable sort on the neighbour label
            nbr.sort_by_key(|&(k, _)| k);
            for &(k, i) in &nbr {
                if k != -1 {
                    old_to_new[cell_faces[celli][i]] = new_face;
                    new_face += 1;
                }
            }
        }
        let n_internal = new_face;
        let mut sizes = vec![0usize; n_patches];
        for f in 0..n_faces {
            if self.region[f] >= 0 {
                sizes[self.region[f] as usize] += 1;
            }
        }
        let mut starts = vec![0usize; n_patches];
        let mut s = n_internal;
        for p in 0..n_patches {
            starts[p] = s;
            s += sizes[p];
        }
        for f in 0..n_faces {
            if self.region[f] >= 0 {
                let p = self.region[f] as usize;
                old_to_new[f] = starts[p];
                starts[p] += 1;
            }
        }
        for (f, &x) in old_to_new.iter().enumerate() {
            assert!(
                x != usize::MAX,
                "polyTopoChange: no position for face {f} (owner {}, neighbour {}, region {}): a boundary face without a patch",
                self.owner[f],
                self.neighbour[f],
                self.region[f]
            );
        }
        let mut faces = vec![Vec::new(); n_faces];
        let mut owner = vec![0usize; n_faces];
        let mut neighbour = vec![0usize; n_internal];
        let mut face_patch = vec![usize::MAX; n_faces];
        for f in 0..n_faces {
            let n = old_to_new[f];
            faces[n] = self.faces[f].clone();
            owner[n] = self.owner[f];
            if self.neighbour[f] >= 0 {
                neighbour[n] = self.neighbour[f] as usize;
            } else {
                face_patch[n] = self.region[f] as usize;
            }
        }
        let mesh = SnapMesh { points: self.points.clone(), faces, owner, neighbour };
        (MergeMesh { mesh, face_patch, n_patches, constraint: constraint.to_vec() }, old_to_new)
    }
}

fn reverse_face(f: &[usize]) -> Vec<usize> {
    let n = f.len();
    let mut r = Vec::with_capacity(n);
    r.push(f[0]);
    for i in 1..n {
        r.push(f[n - i]);
    }
    r
}

fn cells_of(mesh: &SnapMesh, n_cells: usize) -> Vec<Vec<usize>> {
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
    for f in 0..mesh.faces.len() {
        cells[mesh.owner[f]].push(f);
    }
    for f in 0..mesh.n_internal_faces() {
        cells[mesh.neighbour[f]].push(f);
    }
    cells
}

/// Number of cells of a mesh.
pub fn n_cells_of(mesh: &SnapMesh) -> usize {
    let o = mesh.owner.iter().copied().max().map_or(0, |x| x + 1);
    let n = mesh.neighbour.iter().copied().max().map_or(0, |x| x + 1);
    o.max(n)
}

// ---------------------------------------------------------------------------------
// addPatchCellLayer::findDuplicatedPoints
// ---------------------------------------------------------------------------------

/// `addPatchCellLayer::findDuplicatedPoints` for `ppFlip` false, `extrude` true, serial: per mesh
/// face a ring of `meshPoint or -1` (empty when the face has no point to renumber).
fn find_duplicated_points(
    m: &MergeMesh,
    n_cells: usize,
    pp: &LayerPatch,
    is_blocked: &[bool],
    is_dup_patch_point: &[bool],
) -> Vec<Vec<i64>> {
    let mesh = &m.mesh;
    let n_int = mesh.n_internal_faces();
    let point_map: HashMap<usize, usize> = pp.mesh_points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let cells = cells_of(mesh, n_cells);
    let mut base: Vec<Vec<i64>> = vec![Vec::new(); mesh.faces.len()];
    let mut affected = vec![false; n_cells];
    for (pf, f) in pp.local_faces.iter().enumerate() {
        for (fp, &pi) in f.iter().enumerate() {
            if is_dup_patch_point[pi] {
                let mesh_facei = pp.addressing[pf];
                let base_f = &mut base[mesh_facei];
                if base_f.is_empty() {
                    base_f.resize(f.len(), -1);
                }
                base_f[fp] = pp.mesh_points[pi] as i64;
                // extrude (true) != ppFlip (false): only an internal face's neighbour cell is affected
                if mesh_facei < mesh.n_internal_faces() {
                    affected[mesh.neighbour[mesh_facei]] = true;
                }
            }
        }
    }
    let mut min_point_value: HashMap<usize, i64> = HashMap::new();
    loop {
        let mut new_affected = vec![false; n_cells];
        let mut n_changed = 0usize;
        for celli in 0..n_cells {
            if !affected[celli] {
                continue;
            }
            min_point_value.clear();
            for &facei in &cells[celli] {
                let f = &mesh.faces[facei];
                for (fp, &bv) in base[facei].iter().enumerate() {
                    if bv != -1 {
                        match point_map.get(&f[fp]) {
                            Some(&mp) if !is_dup_patch_point[mp] => {}
                            _ => {
                                min_point_value.entry(f[fp]).or_insert(bv);
                            }
                        }
                    }
                }
            }
            for &facei in &cells[celli] {
                let f = &mesh.faces[facei];
                let old_n_changed = n_changed;
                for fp in 0..f.len() {
                    if let Some(&v) = min_point_value.get(&f[fp]) {
                        if let Some(&mp) = point_map.get(&f[fp]) {
                            if !is_dup_patch_point[mp] {
                                continue;
                            }
                        }
                        if base[facei].is_empty() {
                            base[facei].resize(f.len(), -1);
                        }
                        if base[facei][fp] == -1 {
                            base[facei][fp] = v;
                            n_changed += 1;
                        }
                    }
                }
                if !is_blocked[facei] && n_changed > old_n_changed {
                    new_affected[mesh.owner[facei]] = true;
                    if facei < n_int {
                        new_affected[mesh.neighbour[facei]] = true;
                    }
                }
            }
        }
        if n_changed == 0 {
            break;
        }
        affected = new_affected;
    }
    base
}

// ---------------------------------------------------------------------------------
// addPatchCellLayer::setRefinement
// ---------------------------------------------------------------------------------

/// The inputs of `addPatchCellLayer::setRefinement` that `snappyLayerDriver::addLayers` passes.
#[derive(Debug, Clone)]
pub struct AddLayerInput<'a> {
    /// The mesh the patch lives on (at the shrunk position).
    pub m: &'a MergeMesh,
    /// The layer patch.
    pub pp: &'a LayerPatch,
    /// `edgeGlobalFaces` ([`edge_global_faces`]).
    pub egf: &'a [Vec<usize>],
    /// `expansionRatio` per patch point (the driver passes `1 / expansionRatio`).
    pub expansion_ratio: &'a [f64],
    /// `edgePatchID`: patch of the side face of a boundary patch edge, -1 for none.
    pub edge_patch_id: &'a [i64],
    /// `nFaceLayers`.
    pub n_face_layers: &'a [i64],
    /// `nPointLayers`.
    pub n_point_layers: &'a [i64],
    /// `firstLayerDisp`.
    pub first_layer_disp: &'a [V],
}

/// The mesh with layers and the layer bookkeeping of `addPatchCellLayer`.
#[derive(Debug, Clone)]
pub struct AddedLayers {
    /// The new mesh (`polyTopoChange::makeMesh` numbering).
    pub mesh: MergeMesh,
    /// `layerFaces()` in the new numbering: per patch face the original face, then one face per layer
    /// (empty when no layer was added).
    pub layer_faces: Vec<Vec<usize>>,
    /// `addedPoints()` in the new numbering (points are not renumbered).
    pub added_points: Vec<Vec<usize>>,
    /// Number of cells before the layers.
    pub n_old_cells: usize,
    /// New label of every `polyTopoChange` face (old faces first, then added ones in call order).
    pub face_map: Vec<usize>,
}

fn nbr_face(edge_faces: &[Vec<usize>], edgei: usize, facei: usize) -> i64 {
    let ef = &edge_faces[edgei];
    if ef.len() == 2 {
        (if ef[0] != facei { ef[0] } else { ef[1] }) as i64
    } else {
        -1
    }
}

fn add_vertex(pointi: usize, f: &mut Vec<usize>) {
    if f.is_empty() || (*f.last().unwrap() != pointi && f[0] != pointi) {
        f.push(pointi);
    }
}

#[allow(clippy::too_many_arguments)]
fn same_edge_neighbour_apl(
    pp: &LayerPatch,
    egf: &[Vec<usize>],
    added_points: &[Vec<usize>],
    done_edge: &[bool],
    this_glob: usize,
    nbr_glob: i64,
    edgei: usize,
) -> bool {
    let e = pp.edges[edgei];
    !done_edge[edgei]
        && (!added_points[e[0]].is_empty() || !added_points[e[1]].is_empty())
        && nbr_face(egf, edgei, this_glob) == nbr_glob
}

/// `addPatchCellLayer::getEdgeString`: `(startFp, endFp)` or `(-1, -1)`.
fn get_edge_string(
    pp: &LayerPatch,
    egf: &[Vec<usize>],
    done_edge: &[bool],
    added_points: &[Vec<usize>],
    patch_facei: usize,
    global_facei: usize,
) -> (i64, i64) {
    let f_edges = &pp.face_edges[patch_facei];
    let n = f_edges.len();
    let mut start_fp: i64 = -1;
    for (fp, &edgei) in f_edges.iter().enumerate() {
        let e = pp.edges[edgei];
        if !done_edge[edgei] && (!added_points[e[0]].is_empty() || !added_points[e[1]].is_empty()) {
            start_fp = fp as i64;
            break;
        }
    }
    let mut end_fp: i64 = -1;
    if start_fp != -1 {
        let nbr_glob = nbr_face(egf, f_edges[start_fp as usize], global_facei);
        if nbr_glob == -1 {
            end_fp = start_fp;
        } else {
            let init_fp = start_fp as usize;
            let mut sfp = start_fp as usize;
            loop {
                let prev = (sfp + n - 1) % n;
                if prev == init_fp {
                    panic!("addPatchCellLayer::getEdgeString: all edges of face {patch_facei} have the same neighbour {nbr_glob}");
                }
                if !same_edge_neighbour_apl(pp, egf, added_points, done_edge, global_facei, nbr_glob, f_edges[prev]) {
                    break;
                }
                sfp = prev;
            }
            let mut efp = sfp;
            loop {
                let next = (efp + 1) % n;
                if !same_edge_neighbour_apl(pp, egf, added_points, done_edge, global_facei, nbr_glob, f_edges[next]) {
                    break;
                }
                efp = next;
            }
            start_fp = sfp as i64;
            end_fp = efp as i64;
        }
    }
    (start_fp, end_fp)
}

/// `addPatchCellLayer::setRefinement` followed by `polyTopoChange::makeMesh` (serial,
/// `addToMesh`, no zones, no flip, `extrude` true).
pub fn add_patch_cell_layer(inp: &AddLayerInput) -> AddedLayers {
    let (m, pp, egf) = (inp.m, inp.pp, inp.egf);
    let n_pp = pp.local_faces.len();
    let n_pts = pp.n_points();
    let n_old_cells = n_cells_of(&m.mesh);
    assert_eq!(inp.first_layer_disp.len(), n_pts);
    assert_eq!(inp.n_point_layers.len(), n_pts);
    assert_eq!(inp.n_face_layers.len(), n_pp);
    assert!(inp.n_point_layers.iter().all(|&x| x >= 0) && inp.n_face_layers.iter().all(|&x| x >= 0), "illegal number of layers");
    for (edgei, e) in pp.edges.iter().enumerate() {
        if egf[edgei].len() > 2 {
            assert!(
                inp.n_point_layers[e[0]] == 0 && inp.n_point_layers[e[1]] == 0,
                "trying to extrude non-manifold edge {edgei}"
            );
        }
    }
    let mesh_points = &pp.mesh_points;
    let mut tc = TopoBuilder::from_mesh(m, n_old_cells);
    let mut is_blocked = vec![false; m.mesh.faces.len()];
    for &f in &pp.addressing {
        is_blocked[f] = true;
    }
    let patch_id: Vec<i64> = pp.addressing.iter().map(|&f| m.face_patch[f] as i64).collect();
    let mut added_points: Vec<Vec<usize>> =
        (0..n_pts).map(|p| vec![0usize; inp.n_point_layers[p] as usize]).collect();

    // baseFaces
    let mut is_dup_patch_point: Vec<bool> = inp.n_point_layers.iter().map(|&n| n > 0).collect();
    for pf in 0..n_pp {
        if inp.n_face_layers[pf] == 0 {
            for &p in &pp.local_faces[pf] {
                is_dup_patch_point[p] = false;
            }
        }
    }
    let base_faces = find_duplicated_points(m, n_old_cells, pp, &is_blocked, &is_dup_patch_point);

    // points
    for pi in 0..n_pts {
        if !added_points[pi].is_empty() {
            let mut pt = m.mesh.points[mesh_points[pi]];
            let mut disp = inp.first_layer_disp[pi];
            for i in 0..added_points[pi].len() {
                pt = add(pt, disp);
                added_points[pi][i] = tc.add_point(pt);
                disp = scale(inp.expansion_ratio[pi], disp);
            }
        }
    }

    // cells
    let mut added_cells: Vec<Vec<usize>> = vec![Vec::new(); n_pp];
    for pf in 0..n_pp {
        if inp.n_face_layers[pf] > 0 {
            for _ in 0..inp.n_face_layers[pf] {
                added_cells[pf].push(tc.add_cell());
            }
        }
    }

    // layer faces
    let mut layer_faces_poly: Vec<Vec<usize>> = vec![Vec::new(); n_pp];
    for pf in 0..n_pp {
        let mesh_facei = pp.addressing[pf];
        if !added_cells[pf].is_empty() {
            let n_added = added_cells[pf].len();
            layer_faces_poly[pf] = vec![0usize; n_added + 1];
            let f = &pp.local_faces[pf];
            for i in 0..n_added {
                let mut new_face = vec![0usize; f.len()];
                for (fp, &lp) in f.iter().enumerate() {
                    if added_points[lp].is_empty() {
                        new_face[fp] = mesh_points[lp];
                    } else {
                        let offset = added_points[lp].len() - n_added;
                        new_face[fp] = added_points[lp][i + offset];
                    }
                }
                // extrude = true (the `addPatchCellLayer` constructor default)
                let id = if i == n_added - 1 {
                    if patch_id[pf] == -1 {
                        // internal face between the added cell and the original neighbour
                        (m.mesh.neighbour[mesh_facei], added_cells[pf][i] as i64, -1, true)
                    } else {
                        (added_cells[pf][i], -1, patch_id[pf], false)
                    }
                } else {
                    (added_cells[pf][i], added_cells[pf][i + 1] as i64, -1, false)
                };
                let ring = if id.3 { reverse_face(&new_face) } else { new_face };
                layer_faces_poly[pf][i + 1] = tc.add_face(ring, id.0, id.1, id.2);
            }
        }
    }

    // modify the original patch faces
    for pf in 0..n_pp {
        if !added_cells[pf].is_empty() {
            let mesh_facei = pp.addressing[pf];
            layer_faces_poly[pf][0] = mesh_facei;
            let f = &m.mesh.faces[mesh_facei];
            // extrude: the original face becomes the internal face between its owner and layer 0
            tc.modify_face(mesh_facei, f.clone(), m.mesh.owner[mesh_facei], added_cells[pf][0] as i64, -1);
        }
    }

    // edge layers
    let mut edge_layers = vec![0i64; pp.edges.len()];
    for (edgei, e) in pp.edges.iter().enumerate() {
        if inp.n_point_layers[e[0]] == 0 && inp.n_point_layers[e[1]] == 0 {
            edge_layers[edgei] = 0;
        } else {
            let mut x = -1i64;
            for &fi in &pp.edge_faces[edgei] {
                x = x.max(inp.n_face_layers[fi]);
            }
            edge_layers[edgei] = x;
        }
    }

    // side faces
    let mut done_edge = vec![false; pp.edges.len()];
    for pf in 0..n_pp {
        let f_edges = &pp.face_edges[pf];
        for _ in 0..f_edges.len() {
            let (start_fp, end_fp) = get_edge_string(pp, egf, &done_edge, &added_points, pf, pp.addressing[pf]);
            if start_fp == -1 {
                continue;
            }
            let (start_fp, end_fp) = (start_fp as usize, end_fp as usize);
            let f = &pp.local_faces[pf];
            let nf = f.len();
            let size = if end_fp >= start_fp { end_fp - start_fp + 2 } else { end_fp + nf - start_fp + 2 };
            let mut stringed = vec![0usize; size];
            let mut fp = start_fp;
            for i in 0..size - 1 {
                stringed[i] = f[fp];
                done_edge[f_edges[fp]] = true;
                fp = (fp + 1) % nf;
            }
            stringed[size - 1] = f[fp];
            let start_edgei = f_edges[start_fp];
            let num_edge_side_faces = edge_layers[start_edgei];
            for i in 0..num_edge_side_faces {
                let v_end = *stringed.last().unwrap();
                let v_start = stringed[0];
                let mut new_face: Vec<usize> = Vec::new();
                if i == 0 {
                    for &v in &stringed {
                        add_vertex(mesh_points[v], &mut new_face);
                    }
                } else {
                    for &v in &stringed {
                        if !added_points[v].is_empty() {
                            let offset = added_points[v].len() as i64 - num_edge_side_faces;
                            add_vertex(added_points[v][(i + offset - 1) as usize], &mut new_face);
                        } else {
                            add_vertex(mesh_points[v], &mut new_face);
                        }
                    }
                }
                if num_edge_side_faces < added_points[v_end].len() as i64 && i == 0 {
                    let offset = added_points[v_end].len() as i64 - num_edge_side_faces;
                    for ioff in 0..offset {
                        add_vertex(added_points[v_end][ioff as usize], &mut new_face);
                    }
                }
                for &v in stringed.iter().rev() {
                    if !added_points[v].is_empty() {
                        let offset = added_points[v].len() as i64 - num_edge_side_faces;
                        add_vertex(added_points[v][(i + offset) as usize], &mut new_face);
                    } else {
                        add_vertex(mesh_points[v], &mut new_face);
                    }
                }
                if num_edge_side_faces < added_points[v_start].len() as i64 && i == 0 {
                    let offset = added_points[v_start].len() as i64 - num_edge_side_faces;
                    for ioff in (0..offset).rev() {
                        add_vertex(added_points[v_start][ioff as usize], &mut new_face);
                    }
                }
                if new_face.len() >= 3 {
                    // extrude_ (true) != ppFlip[patchFacei] (false): not reversed
                    let nbr_facei = nbr_face(&pp.edge_faces, start_edgei, pf);
                    add_side_face(
                        &mut tc,
                        &added_cells,
                        new_face,
                        inp.edge_patch_id[start_edgei],
                        pf,
                        nbr_facei,
                        i,
                        num_edge_side_faces,
                    );
                }
            }
        }
    }

    // the faces of the old cells use the last added point
    let n_int = m.mesh.n_internal_faces();
    for facei in 0..m.mesh.faces.len() {
        let f = &m.mesh.faces[facei];
        let base_f = &base_faces[facei];
        if is_blocked[facei] || base_f.is_empty() {
            continue;
        }
        let mut new_face = f.clone();
        for (fp, &mp) in f.iter().enumerate() {
            if base_f[fp] != -1 {
                if let Some(pi) = pp.mesh_points.iter().position(|&x| x == mp) {
                    new_face[fp] = *added_points[pi].last().expect("duplicated point without added points");
                }
            }
        }
        if facei < n_int {
            tc.modify_face(facei, new_face, m.mesh.owner[facei], m.mesh.neighbour[facei] as i64, -1);
        } else {
            tc.modify_face(facei, new_face, m.mesh.owner[facei], -1, m.face_patch[facei] as i64);
        }
    }
    // `if (!extrude_)` (modifyPoint of the other duplicated points) is skipped: extrude is true.

    let (new_mesh, face_map) = tc.make_mesh(m.n_patches, &m.constraint);
    let layer_faces: Vec<Vec<usize>> = layer_faces_poly
        .iter()
        .map(|lf| lf.iter().map(|&f| face_map[f]).collect())
        .collect();
    AddedLayers { mesh: new_mesh, layer_faces, added_points, n_old_cells, face_map }
}

/// `addPatchCellLayer::addSideFace` for a mesh without zones.
#[allow(clippy::too_many_arguments)]
fn add_side_face(
    tc: &mut TopoBuilder,
    added_cells: &[Vec<usize>],
    new_face: Vec<usize>,
    new_patch_id: i64,
    own_facei: usize,
    nbr_facei: i64,
    layeri: i64,
    num_edge_faces: i64,
) {
    if nbr_facei == -1 {
        let own_n = added_cells[own_facei].len() as i64;
        let layer_own = if own_n < num_edge_faces {
            let offset = num_edge_faces - own_n;
            if layeri <= offset {
                0
            } else {
                layeri - offset
            }
        } else {
            layeri
        };
        tc.add_face(new_face, added_cells[own_facei][layer_own as usize], -1, new_patch_id);
    } else {
        let nbr = nbr_facei as usize;
        let (own_n, nbr_n) = (added_cells[own_facei].len() as i64, added_cells[nbr].len() as i64);
        let (layer_own, layer_nbr);
        if own_n > nbr_n {
            let offset = own_n - nbr_n;
            layer_own = layeri;
            layer_nbr = if layeri <= offset { 0 } else { layeri - offset };
        } else if nbr_n > own_n {
            let offset = nbr_n - own_n;
            layer_nbr = layeri;
            layer_own = if layeri <= offset { 0 } else { layeri - offset };
        } else {
            layer_nbr = layeri;
            layer_own = layeri;
        }
        tc.add_face(
            new_face,
            added_cells[own_facei][layer_own as usize],
            added_cells[nbr][layer_nbr as usize] as i64,
            -1,
        );
    }
}

/// `snappyLayerDriver::determineSidePatches` / `addPatchCellLayer::calcExtrudeInfo` for a serial mesh
/// without zones: for every patch edge used by one patch face, the patch of the lowest-numbered
/// non-patch boundary face using the same mesh edge (-1 if none, and for edges with two patch faces).
pub fn determine_side_patches(m: &MergeMesh, pp: &LayerPatch) -> Vec<i64> {
    let mut is_pp_face = vec![false; m.mesh.faces.len()];
    for &f in &pp.addressing {
        is_pp_face[f] = true;
    }
    let mut edge_of: HashMap<(usize, usize), usize> = HashMap::new();
    for (ei, e) in pp.edges.iter().enumerate() {
        let (a, b) = (pp.mesh_points[e[0]], pp.mesh_points[e[1]]);
        edge_of.insert((a.min(b), a.max(b)), ei);
    }
    let mut to_patch = vec![-1i64; pp.edges.len()];
    for facei in m.mesh.n_internal_faces()..m.mesh.faces.len() {
        if is_pp_face[facei] {
            continue;
        }
        let f = &m.mesh.faces[facei];
        let mut prev = *f.last().unwrap();
        for &p in f {
            if let Some(&ei) = edge_of.get(&(prev.min(p), prev.max(p))) {
                if to_patch[ei] == -1 {
                    to_patch[ei] = m.face_patch[facei] as i64;
                }
            }
            prev = p;
        }
    }
    (0..pp.edges.len()).map(|ei| if pp.edge_faces[ei].len() == 2 { -1 } else { to_patch[ei] }).collect()
}

// ---------------------------------------------------------------------------------
// getLayerCellsFaces / checkAndUnmark
// ---------------------------------------------------------------------------------

/// `addPatchCellLayer::addedCells(mesh, layerFaces)`: per patch face the cell of each layer.
pub fn added_cells(mesh: &SnapMesh, layer_faces: &[Vec<usize>]) -> Vec<Vec<usize>> {
    layer_faces
        .iter()
        .map(|lf| if lf.is_empty() { Vec::new() } else { (0..lf.len() - 1).map(|i| mesh.owner[lf[i + 1]]).collect() })
        .collect()
}

/// `snappyLayerDriver::getLayerCellsFaces`: `(cellNLayers, faceRealThickness)` on the layer mesh;
/// `old_real_thickness` is `avgPointData(pp, mag(patchDisp))` per patch face.
pub fn get_layer_cells_faces(
    mesh: &SnapMesh,
    layer_faces: &[Vec<usize>],
    old_real_thickness: &[f64],
) -> (Vec<i64>, Vec<f64>) {
    let mut cell_n_layers = vec![0i64; n_cells_of(mesh)];
    let mut face_real = vec![0.0f64; mesh.faces.len()];
    let cells = added_cells(mesh, layer_faces);
    for (pf, added) in cells.iter().enumerate() {
        let layer = &layer_faces[pf];
        if !layer.is_empty() {
            for &c in added {
                cell_n_layers[c] = layer.len() as i64 - 1;
            }
        }
    }
    for (pf, layer) in layer_faces.iter().enumerate() {
        if !layer.is_empty() {
            for &f in &layer[1..] {
                face_real[f] = old_real_thickness[pf];
            }
        }
    }
    (cell_n_layers, face_real)
}

/// `snappyLayerDriver::checkAndUnmark` (no baffles, no additional reporting): the faces of the
/// layer mesh that fail the mesh quality unmark the extrusion of the patch face whose added cells
/// use them. Returns the number of unmarked patch faces and the quality check.
pub fn check_and_unmark(
    layers: &AddedLayers,
    quality: &EsiQualityControls,
    pp: &LayerPatch,
    st: &mut ExtrudeState,
) -> (usize, EsiCheckResult) {
    let mesh = &layers.mesh.mesh;
    let geom = esi_geometry(mesh);
    let all: Vec<usize> = (0..mesh.faces.len()).collect();
    let check = esi_check_mesh(mesh, &geom, quality, &all);
    let mut is_wrong = vec![false; mesh.faces.len()];
    for &f in &check.wrong_faces {
        is_wrong[f] = true;
    }
    let cells = added_cells(mesh, &layers.layer_faces);
    let mut n_changed = 0usize;
    for (pf, f_cells) in cells.iter().enumerate() {
        let uses = f_cells.iter().any(|&c| geom.cell_faces[c].iter().any(|&f| is_wrong[f]));
        if uses && st.unmark_face(&pp.local_faces[pf]) {
            n_changed += 1;
        }
    }
    (n_changed, check)
}

/// `avgPointData(pp, data)`: per patch face the average of the point data of its vertices
/// (`snappyLayerDriver::avgPointData`: sum over the face points then division by the size).
pub fn avg_point_data_faces(pp: &LayerPatch, point_data: &[f64]) -> Vec<f64> {
    pp.local_faces
        .iter()
        .map(|f| {
            let mut s = 0.0;
            for &p in f {
                s += point_data[p];
            }
            s / f.len() as f64
        })
        .collect()
}

/// Displacement `finalDisp = ratio * patchDisp` helper used by the driver (`scale` re-export for tests).
pub fn scaled_disp(ratio: f64, d: V) -> V {
    scale(ratio, d)
}

/// Difference of two points (kept so the module has no unused import when tests are off).
pub fn point_diff(a: V, b: V) -> V {
    sub(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One hex cell (cell 0), all six faces boundary faces of patch 0, bottom face first.
    fn one_hex() -> MergeMesh {
        let points: Vec<V> = vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [1., 0., 1.],
            [1., 1., 1.],
            [0., 1., 1.],
        ];
        let faces = vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 4, 7, 3],
            vec![1, 2, 6, 5],
            vec![0, 1, 5, 4],
            vec![3, 7, 6, 2],
        ];
        let mesh = SnapMesh { points, faces, owner: vec![0; 6], neighbour: vec![] };
        MergeMesh::from_patch_starts(mesh, &[0, 6])
    }

    #[test]
    fn one_layer_under_one_face_adds_cell_points_and_side_faces() {
        let m = one_hex();
        let pp = LayerPatch::from_faces(&m, vec![0]);
        let egf = edge_global_faces(&pp);
        let side = determine_side_patches(&m, &pp);
        // the four edges of the face have a boundary neighbour face of patch 0
        assert_eq!(side, vec![0, 0, 0, 0]);
        let disp = vec![[0.0, 0.0, 0.1]; 4];
        let ratio = vec![1.0; 4];
        let inp = AddLayerInput {
            m: &m,
            pp: &pp,
            egf: &egf,
            expansion_ratio: &ratio,
            edge_patch_id: &side,
            n_face_layers: &[1],
            n_point_layers: &[1, 1, 1, 1],
            first_layer_disp: &disp,
        };
        let out = add_patch_cell_layer(&inp);
        assert_eq!(out.mesh.mesh.points.len(), 12);
        assert_eq!(n_cells_of(&out.mesh.mesh), 2);
        // 6 old faces + 1 outer boundary face + 4 side faces; the original face is the interface
        assert_eq!(out.mesh.mesh.faces.len(), 11);
        assert_eq!(out.mesh.mesh.n_internal_faces(), 1);
        assert_eq!(out.layer_faces[0].len(), 2);
        let iface = out.layer_faces[0][0];
        assert_eq!((out.mesh.mesh.owner[iface], out.mesh.mesh.neighbour[iface]), (0, 1));
        let outer = out.layer_faces[0][1];
        assert_eq!(out.mesh.mesh.owner[outer], 1);
        assert!(outer >= out.mesh.mesh.n_internal_faces());
        let cells = cells_of(&out.mesh.mesh, 2);
        assert_eq!(cells[1].len(), 6);
        // wrong-quality bookkeeping on the result is reachable
        let cl = get_layer_cells_faces(&out.mesh.mesh, &out.layer_faces, &[0.1]);
        assert_eq!(cl.0[1], 1);
        assert!(cl.1[outer] > 0.0);
        assert!((avg_point_data_faces(&pp, &[1.0, 2.0, 3.0, 4.0])[0] - 2.5).abs() < 1e-15);
        assert_eq!(scaled_disp(2.0, [1.0, 0.0, 0.0])[0], 2.0);
        assert_eq!(point_diff([1.0; 3], [0.5; 3]), [0.5; 3]);
    }

    #[test]
    fn truncation_unmarks_pinched_faces_and_counts_extrusion() {
        // a strip of two quads sharing an edge, extruded at 3 of 6 points that are not
        // consecutive on the second quad: the quad is unmarked (non-consecutive vertices)
        let points: Vec<V> =
            vec![[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [0., 1., 0.], [1., 1., 0.], [2., 1., 0.]];
        let faces = vec![vec![0, 3, 4, 1], vec![1, 4, 5, 2]];
        let mesh = SnapMesh { points, faces, owner: vec![0, 1], neighbour: vec![] };
        let m = MergeMesh::from_patch_starts(mesh, &[0, 2]);
        let pp = LayerPatch::new(&m, &[0]);
        let egf = edge_global_faces(&pp);
        let mut st = ExtrudeState::new(pp.n_points());
        for p in 0..pp.n_points() {
            st.disp[p] = [0.0, 0.0, 1.0];
            st.n_layers[p] = 1;
        }
        // quad 1 has local points 1,4,5,2 (first appearance: 0,3,4,1,5,2): leave its vertices 1 and
        // 5 unmarked (opposite corners), which `n_trans > 1` removes
        let local_of = |mp: usize| pp.mesh_points.iter().position(|&x| x == mp).unwrap();
        st.unmark_point(local_of(1));
        st.unmark_point(local_of(5));
        let min_t = vec![0.01; pp.n_points()];
        let rep = truncate_displacement(&m, &pp, &egf, &min_t, &[], &mut st);
        assert!(rep.n_changed > 0 && rep.passes.last() == Some(&(0, 0)));
        assert_eq!(count_extrusion(&pp, &st.status), count_extrusion(&pp, &st.status));
        // a face-set point is unmarked through the illegal-face path
        let mut st2 = ExtrudeState::new(pp.n_points());
        for p in 0..pp.n_points() {
            st2.disp[p] = [0.0, 0.0, 1.0];
            st2.n_layers[p] = 1;
        }
        // no boundary face outside the patch exists; a patch face as "illegal" unmarks its points
        let rep2 = truncate_displacement(&m, &pp, &egf, &min_t, &[1], &mut st2);
        assert!(rep2.n_changed >= 4);
        let (npl, nfl) = setup_layer_info_truncation(&pp, &st2.n_layers, &st2.status, 0);
        assert_eq!(nfl[1], 0);
        assert!(pp.local_faces[1].iter().all(|&p| npl[p] == 0 || pp.local_faces[0].contains(&p)));
        let (npl, nfl) = setup_layer_info_truncation(&pp, &[1, 1, 1, 1, 1, 1], &st.status, -1);
        assert_eq!(npl, vec![1; 6]);
        assert_eq!(nfl, vec![1, 1]);
        let q = EsiQualityControls::default();
        let _ = &q;
    }
}
