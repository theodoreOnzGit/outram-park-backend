// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2016-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.
//
// Upstream: OpenFOAM.com ESI v2506, `src/mesh/snappyHexMesh/snappyHexMeshDriver/`
// `snappyLayerDriver.C` (`setNumLayers`, `handleNonStringConnected`, `handleNonManifolds`,
// `handleFeatureAngle`, `handleWarpedFaces`, `growNoExtrusion`, `calculateLayerThickness`,
// `unmarkExtrusion`) and `layerParameters/layerParameters.C`.

//! gh:#1467 round 8: the front of ESI `snappyLayerDriver::addLayers`: `layerParameters` (thickness
//! models), the layer patch (`meshRefinement::makePatch`), and the per-point extrusion set-up of
//! `setPointNumLayers` (`setNumLayers`, `handleNonStringConnected`, `handleNonManifolds`,
//! `handleFeatureAngle`, `handleWarpedFaces`, `growNoExtrusion`) and `calculateLayerThickness`.
//!
//! Opt-in: nothing else calls it. The Foundation-style paths (`layers.rs`,
//! `outram_park_fork_cfmesh::snappy_layers`) are untouched and stay callable.
//!
//! **Serial only** (`syncTools` calls are identities), **no faceZones** (no baffles, no point
//! duplication). The patch-name regex of the `layers {}` dictionary is not ported: the caller
//! gives the per-patch overrides. The primitive-patch **edge order** is first appearance, not
//! `PrimitivePatch::calcAddressing`'s internal-then-boundary order; nothing in this module depends
//! on edge order (every use is an order-independent set operation), the later stages
//! (`addPatchCellLayer`) will.
//!
//! Methodology, predictions P49-P51 and the results: `verification_and_validation/
//! snappy_hex_mesh_esi_v2506/README.md`, "Round 8".

use std::collections::{BTreeMap, HashMap};

use super::esi_merge::MergeMesh;
use super::esi_snap::{div, dot, face_centres_areas, mag, scale, sub, V};

/// OpenFOAM `GREAT` (double precision).
pub const GREAT: f64 = 1e15;
/// OpenFOAM `VGREAT`.
pub const VGREAT: f64 = 1e300;
/// OpenFOAM `SMALL`.
pub const SMALL: f64 = 1e-15;
const ROOT_V_SMALL: f64 = 1e-150;

// ---------------------------------------------------------------------------------
// layerParameters
// ---------------------------------------------------------------------------------

/// `layerParameters::thicknessModelType`; the discriminants are the enum's, so `max()` of two
/// models (as `calculateLayerThickness` does across patches) is the same comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThicknessModel {
    /// `firstAndOverall`.
    FirstAndTotal = 0,
    /// `firstAndExpansion`.
    FirstAndExpansion = 1,
    /// `finalAndOverall`.
    FinalAndTotal = 2,
    /// `finalAndExpansion`.
    FinalAndExpansion = 3,
    /// `overallAndExpansion`.
    TotalAndExpansion = 4,
    /// `firstAndRelativeFinal`.
    FirstAndRelativeFinal = 5,
}

/// `layerParameters::finalLayerThicknessRatio`.
pub fn final_layer_thickness_ratio(n_layers: i64, expansion_ratio: f64) -> f64 {
    if n_layers > 0 {
        if (expansion_ratio - 1.0).abs() < SMALL {
            1.0 / n_layers as f64
        } else {
            pow(expansion_ratio, n_layers - 1) * (1.0 - expansion_ratio) / (1.0 - pow(expansion_ratio, n_layers))
        }
    } else {
        0.0
    }
}

fn pow(x: f64, n: i64) -> f64 {
    // std::pow(double, int) promotes to pow(double, double).
    use petir::mathf::RealMath;
    x.r_powf(n as f64)
}

/// `layerParameters::layerExpansionRatio(n, totalOverFirst)`: the Newton iteration.
pub fn layer_expansion_ratio_newton(n: i64, total_over_first: f64) -> f64 {
    use petir::mathf::RealMath;
    if n <= 1 {
        return 1.0;
    }
    let max_iters = 20;
    let tol = 1e-8;
    let nf = n as f64;
    if (nf - total_over_first).abs() < tol {
        return 1.0;
    }
    let (min_r, max_r);
    if total_over_first < nf {
        min_r = 0.0;
        max_r = (total_over_first / nf).r_powf(1.0 / (nf - 1.0));
    } else {
        min_r = (total_over_first / nf).r_powf(1.0 / (nf - 1.0));
        max_r = total_over_first / (nf - 1.0);
    }
    let mut r = 0.5 * (min_r + max_r);
    for _ in 0..max_iters {
        let prevr = r;
        let fx = r.r_powf(nf) - total_over_first * r - (1.0 - total_over_first);
        let dfx = nf * r.r_powf(nf - 1.0) - total_over_first;
        r -= fx / dfx;
        if (r - prevr).abs() < tol {
            break;
        }
    }
    r
}

/// The four numbers a spec carries (`-123` when unset, as `layerParameters`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerNumbers {
    /// `firstLayerThickness`.
    pub first: f64,
    /// `finalLayerThickness`.
    pub final_: f64,
    /// `thickness` (overall).
    pub total: f64,
    /// `expansionRatio`.
    pub expansion: f64,
}

/// `layerParameters::layerThickness(spec, nLayers, first, final, total, expansionRatio)`.
pub fn layer_thickness_spec(spec: ThicknessModel, n_layers: i64, n: LayerNumbers) -> f64 {
    let nl = n_layers as f64;
    match spec {
        ThicknessModel::FirstAndTotal | ThicknessModel::FinalAndTotal | ThicknessModel::TotalAndExpansion => n.total,
        ThicknessModel::FirstAndExpansion => {
            if (n.expansion - 1.0).abs() < SMALL {
                n.first * nl
            } else {
                n.first * (1.0 - pow(n.expansion, n_layers)) / (1.0 - n.expansion)
            }
        }
        ThicknessModel::FinalAndExpansion => {
            if (n.expansion - 1.0).abs() < SMALL {
                n.final_ * nl
            } else {
                let inv = 1.0 / n.expansion;
                n.final_ * (1.0 - pow(inv, n_layers)) / (1.0 - inv)
            }
        }
        ThicknessModel::FirstAndRelativeFinal => {
            if (n.expansion - 1.0).abs() < SMALL {
                n.first * nl
            } else {
                let ratio = layer_expansion_ratio_spec(spec, n_layers, n);
                if (ratio - 1.0).abs() < SMALL {
                    n.first * nl
                } else {
                    n.first * (1.0 - pow(ratio, n_layers)) / (1.0 - ratio)
                }
            }
        }
    }
}

/// `layerParameters::layerExpansionRatio(spec, ...)`.
pub fn layer_expansion_ratio_spec(spec: ThicknessModel, n_layers: i64, n: LayerNumbers) -> f64 {
    use petir::mathf::RealMath;
    match spec {
        ThicknessModel::FirstAndExpansion | ThicknessModel::FinalAndExpansion | ThicknessModel::TotalAndExpansion => {
            n.expansion
        }
        ThicknessModel::FirstAndTotal => {
            if n.first < SMALL {
                1.0
            } else {
                layer_expansion_ratio_newton(n_layers, n.total / n.first)
            }
        }
        ThicknessModel::FinalAndTotal => {
            if n.final_ < SMALL {
                1.0
            } else {
                1.0 / layer_expansion_ratio_newton(n_layers, n.total / n.final_)
            }
        }
        ThicknessModel::FirstAndRelativeFinal => {
            if n.first < SMALL || n_layers <= 1 {
                1.0
            } else {
                (n.final_ / n.first).r_powf(1.0 / (n_layers - 1) as f64)
            }
        }
    }
}

/// `layerParameters::firstLayerThickness(spec, ...)`.
pub fn first_layer_thickness_spec(spec: ThicknessModel, n_layers: i64, n: LayerNumbers) -> f64 {
    match spec {
        ThicknessModel::FirstAndExpansion | ThicknessModel::FirstAndTotal | ThicknessModel::FirstAndRelativeFinal => {
            n.first
        }
        ThicknessModel::FinalAndExpansion => {
            if n.expansion < SMALL {
                0.0
            } else {
                n.final_ * pow(1.0 / n.expansion, n_layers - 1)
            }
        }
        ThicknessModel::FinalAndTotal => {
            let r = layer_expansion_ratio_spec(spec, n_layers, n);
            n.final_ / pow(r, n_layers - 1)
        }
        ThicknessModel::TotalAndExpansion => {
            let r = final_layer_thickness_ratio(n_layers, n.expansion);
            let final_thickness = r * n.total;
            final_thickness / pow(n.expansion, n_layers - 1)
        }
    }
}

/// `layerParameters::layerThickness(nLayers, layerThickness, expansionRatio, layerStart,
/// layerSize)`: the thickness of a slice of the layers. Panics on an illegal slice, as ESI aborts.
pub fn layer_thickness_slice(
    n_layers: i64,
    layer_thickness: f64,
    expansion_ratio: f64,
    layer_start: i64,
    layer_size: i64,
) -> f64 {
    if layer_size == 0 || n_layers == 0 {
        0.0
    } else if layer_size > n_layers || layer_start >= n_layers {
        panic!("Illegal input for slice of layer: nLayers {n_layers} size {layer_size} start {layer_start}")
    } else if (expansion_ratio - 1.0).abs() < SMALL {
        layer_thickness * layer_size as f64 / n_layers as f64
    } else {
        let first = final_layer_thickness_ratio(n_layers, expansion_ratio) * layer_thickness
            / pow(expansion_ratio, n_layers - 1);
        let start_thickness = first * pow(expansion_ratio, layer_start);
        start_thickness * (1.0 - pow(expansion_ratio, layer_size)) / (1.0 - expansion_ratio)
    }
}

/// `layerParameters::calculateLayerParameters` for one patch: fills the numbers the spec does
/// not carry.
pub fn calculate_layer_parameters(spec: ThicknessModel, n_layers: i64, n: &mut LayerNumbers) {
    match spec {
        ThicknessModel::FirstAndTotal => {
            let probe = LayerNumbers { first: n.first, final_: VGREAT, total: n.total, expansion: VGREAT };
            n.expansion = layer_expansion_ratio_spec(spec, n_layers, probe);
            n.final_ = n.total * final_layer_thickness_ratio(n_layers, n.expansion);
        }
        ThicknessModel::FirstAndExpansion => {
            let probe = LayerNumbers { first: n.first, final_: VGREAT, total: VGREAT, expansion: n.expansion };
            n.total = layer_thickness_spec(spec, n_layers, probe);
            n.final_ = n.total * final_layer_thickness_ratio(n_layers, n.expansion);
        }
        ThicknessModel::FinalAndTotal => {
            let probe = LayerNumbers { first: VGREAT, final_: VGREAT, total: n.total, expansion: VGREAT };
            n.first = first_layer_thickness_spec(spec, n_layers, probe);
            let probe = LayerNumbers { first: VGREAT, final_: n.final_, total: n.total, expansion: VGREAT };
            n.expansion = layer_expansion_ratio_spec(spec, n_layers, probe);
        }
        ThicknessModel::FinalAndExpansion => {
            let probe = LayerNumbers { first: VGREAT, final_: n.final_, total: VGREAT, expansion: n.expansion };
            n.first = first_layer_thickness_spec(spec, n_layers, probe);
            n.total = layer_thickness_spec(spec, n_layers, probe);
        }
        ThicknessModel::TotalAndExpansion => {
            let probe = LayerNumbers { first: VGREAT, final_: n.final_, total: VGREAT, expansion: n.expansion };
            n.first = first_layer_thickness_spec(spec, n_layers, probe);
            n.final_ = n.total * final_layer_thickness_ratio(n_layers, n.expansion);
        }
        ThicknessModel::FirstAndRelativeFinal => {
            let probe = LayerNumbers { first: n.first, final_: n.final_, total: VGREAT, expansion: VGREAT };
            n.total = layer_thickness_spec(spec, n_layers, probe);
            n.expansion = layer_expansion_ratio_spec(spec, n_layers, probe);
        }
    }
}

/// The global `addLayersControls` entries (the ones this port reads).
#[derive(Debug, Clone)]
pub struct LayerDict {
    /// `relativeSizes`.
    pub relative_sizes: bool,
    /// The global thickness numbers; `-123` for those the global spec does not carry.
    pub numbers: LayerNumbers,
    /// The model chosen by the global entries (`thicknessModel` or inferred).
    pub model: ThicknessModel,
    /// `minThickness`.
    pub min_thickness: f64,
    /// `featureAngle`.
    pub feature_angle: f64,
    /// `nGrow`.
    pub n_grow: usize,
    /// `maxFaceThicknessRatio`.
    pub max_face_thickness_ratio: f64,
    /// `nBufferCellsNoExtrude`.
    pub n_buffer_cells_no_extrude: usize,
    /// `nLayerIter`.
    pub n_layer_iter: usize,
    /// `nRelaxedIter`.
    pub n_relaxed_iter: usize,
    /// `nOuterIter` (default 1).
    pub n_outer_iter: usize,
}

/// What one `layers { "pattern" { ... } }` entry sets on the patches it matches (the caller
/// resolves the regex; `None` leaves the global value). Only the same-model overrides of
/// `layerParameters` are modelled (`thicknessModel` per patch is not).
#[derive(Debug, Clone, Default)]
pub struct PatchLayerDict {
    /// `nSurfaceLayers` (required by ESI).
    pub n_surface_layers: i64,
    /// `firstLayerThickness`.
    pub first: Option<f64>,
    /// `finalLayerThickness`.
    pub final_: Option<f64>,
    /// `thickness`.
    pub total: Option<f64>,
    /// `expansionRatio`.
    pub expansion: Option<f64>,
    /// `minThickness`.
    pub min_thickness: Option<f64>,
    /// `relativeSizes`.
    pub relative_sizes: Option<bool>,
}

/// `layerParameters`, per patch.
#[derive(Debug, Clone)]
pub struct LayerParameters {
    /// The global entries.
    pub dict: LayerDict,
    /// `numLayers()` (-1 for a patch without layers).
    pub num_layers: Vec<i64>,
    /// `relativeSizes()`.
    pub relative_sizes: Vec<bool>,
    /// `layerModels()`.
    pub layer_models: Vec<ThicknessModel>,
    /// Per patch `firstLayerThickness`, `finalLayerThickness`, `thickness`, `expansionRatio`
    /// (after `calculateLayerParameters`).
    pub numbers: Vec<LayerNumbers>,
    /// `minThickness()`.
    pub min_thickness: Vec<f64>,
}

impl LayerParameters {
    /// The constructor: `overrides` are `(patch, entry)` pairs in dictionary order (a later entry
    /// matching the same patch overrides an earlier one, as ESI's loop does).
    pub fn new(dict: &LayerDict, n_patches: usize, overrides: &[(usize, PatchLayerDict)]) -> Self {
        let mut p = Self {
            dict: dict.clone(),
            num_layers: vec![-1; n_patches],
            relative_sizes: vec![dict.relative_sizes; n_patches],
            layer_models: vec![dict.model; n_patches],
            numbers: vec![dict.numbers; n_patches],
            min_thickness: vec![dict.min_thickness; n_patches],
        };
        for (patch, d) in overrides {
            let pi = *patch;
            p.num_layers[pi] = d.n_surface_layers;
            let n = &mut p.numbers[pi];
            match p.layer_models[pi] {
                ThicknessModel::FirstAndTotal => {
                    set(&mut n.first, d.first);
                    set(&mut n.total, d.total);
                }
                ThicknessModel::FirstAndExpansion => {
                    set(&mut n.first, d.first);
                    set(&mut n.expansion, d.expansion);
                }
                ThicknessModel::FinalAndTotal => {
                    set(&mut n.final_, d.final_);
                    set(&mut n.total, d.total);
                }
                ThicknessModel::FinalAndExpansion => {
                    set(&mut n.final_, d.final_);
                    set(&mut n.expansion, d.expansion);
                }
                ThicknessModel::TotalAndExpansion => {
                    set(&mut n.total, d.total);
                    set(&mut n.expansion, d.expansion);
                }
                ThicknessModel::FirstAndRelativeFinal => {
                    set(&mut n.first, d.first);
                    set(&mut n.final_, d.final_);
                }
            }
            set(&mut p.min_thickness[pi], d.min_thickness);
            if let Some(r) = d.relative_sizes {
                p.relative_sizes[pi] = r;
            }
        }
        for pi in 0..n_patches {
            let spec = p.layer_models[pi];
            let nl = p.num_layers[pi];
            calculate_layer_parameters(spec, nl, &mut p.numbers[pi]);
        }
        p
    }
}

fn set(x: &mut f64, v: Option<f64>) {
    if let Some(v) = v {
        *x = v;
    }
}

// ---------------------------------------------------------------------------------
// The layer patch (indirectPrimitivePatch of the layer patches)
// ---------------------------------------------------------------------------------

/// `meshRefinement::makePatch(mesh, patchIDs)`: the faces of the patches, in patch order, as an
/// `indirectPrimitivePatch` with its addressing.
#[derive(Debug, Clone)]
pub struct LayerPatch {
    /// `addressing()`: mesh face of each patch face.
    pub addressing: Vec<usize>,
    /// `meshPoints()`: mesh point of each local point, in order of first appearance.
    pub mesh_points: Vec<usize>,
    /// `localFaces()`.
    pub local_faces: Vec<Vec<usize>>,
    /// `pointFaces()`.
    pub point_faces: Vec<Vec<usize>>,
    /// `edges()` (local points, smaller first as `edge`'s constructor does not sort: kept as
    /// `(first, second)` of first appearance), in order of first appearance.
    pub edges: Vec<[usize; 2]>,
    /// `edgeFaces()`.
    pub edge_faces: Vec<Vec<usize>>,
    /// `faceNormals()` (unit), on the mesh points at construction.
    pub face_normals: Vec<V>,
}

/// `face::unitNormal(points)`.
pub fn unit_normal(points: &[V], f: &[usize]) -> V {
    let n = crate::check_mesh::esi_face_area_normal(points, f);
    let s = mag(n);
    if s < ROOT_V_SMALL {
        [0.0; 3]
    } else {
        div(n, s)
    }
}

impl LayerPatch {
    /// Build the patch over `patch_ids` of `m` and compute its normals on `m.mesh.points`.
    pub fn new(m: &MergeMesh, patch_ids: &[usize]) -> Self {
        let starts = m.patch_starts();
        let mut addressing = Vec::new();
        for &p in patch_ids {
            addressing.extend(starts[p]..starts[p + 1]);
        }
        Self::from_faces(m, addressing)
    }

    /// From an explicit list of mesh faces.
    pub fn from_faces(m: &MergeMesh, addressing: Vec<usize>) -> Self {
        let mut local_of: HashMap<usize, usize> = HashMap::new();
        let mut mesh_points = Vec::new();
        for &f in &addressing {
            for &p in &m.mesh.faces[f] {
                local_of.entry(p).or_insert_with(|| {
                    mesh_points.push(p);
                    mesh_points.len() - 1
                });
            }
        }
        let local_faces: Vec<Vec<usize>> =
            addressing.iter().map(|&f| m.mesh.faces[f].iter().map(|p| local_of[p]).collect()).collect();
        let mut point_faces = vec![Vec::new(); mesh_points.len()];
        for (fi, f) in local_faces.iter().enumerate() {
            for &p in f {
                point_faces[p].push(fi);
            }
        }
        let mut edge_of: HashMap<(usize, usize), usize> = HashMap::new();
        let mut edges: Vec<[usize; 2]> = Vec::new();
        let mut edge_faces: Vec<Vec<usize>> = Vec::new();
        for (fi, f) in local_faces.iter().enumerate() {
            for k in 0..f.len() {
                let (a, b) = (f[k], f[(k + 1) % f.len()]);
                let key = (a.min(b), a.max(b));
                let ei = *edge_of.entry(key).or_insert_with(|| {
                    edges.push([a, b]);
                    edge_faces.push(Vec::new());
                    edges.len() - 1
                });
                edge_faces[ei].push(fi);
            }
        }
        let face_normals: Vec<V> = addressing.iter().map(|&f| unit_normal(&m.mesh.points, &m.mesh.faces[f])).collect();
        Self { addressing, mesh_points, local_faces, point_faces, edges, edge_faces, face_normals }
    }

    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.mesh_points.len()
    }
}

// ---------------------------------------------------------------------------------
// Extrusion state
// ---------------------------------------------------------------------------------

/// `snappyLayerDriver::extrudeMode` (NOEXTRUDE is the minimum, as ESI relies on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtrudeMode {
    /// No extrusion at this point.
    NoExtrude = 0,
    /// Extrude.
    Extrude = 1,
    /// Extrude, and remove it later.
    ExtrudeRemove = 2,
}

/// The per-point arrays `patchDisp`, `patchNLayers`, `extrudeStatus` of `addLayers`.
#[derive(Debug, Clone)]
pub struct ExtrudeState {
    /// `patchDisp`.
    pub disp: Vec<V>,
    /// `patchNLayers`.
    pub n_layers: Vec<i64>,
    /// `extrudeStatus`.
    pub status: Vec<ExtrudeMode>,
}

impl ExtrudeState {
    /// `patchDisp = (GREAT,GREAT,GREAT)`, `patchNLayers = 0`, `EXTRUDE`, for `n` points.
    pub fn new(n: usize) -> Self {
        Self { disp: vec![[GREAT; 3]; n], n_layers: vec![0; n], status: vec![ExtrudeMode::Extrude; n] }
    }

    /// `unmarkExtrusion(patchPointi, ...)`.
    pub fn unmark_point(&mut self, i: usize) -> bool {
        match self.status[i] {
            ExtrudeMode::Extrude | ExtrudeMode::ExtrudeRemove => {
                self.status[i] = ExtrudeMode::NoExtrude;
                self.n_layers[i] = 0;
                self.disp[i] = [0.0; 3];
                true
            }
            ExtrudeMode::NoExtrude => false,
        }
    }

    /// `unmarkExtrusion(localFace, ...)`: true when anything was unset.
    pub fn unmark_face(&mut self, f: &[usize]) -> bool {
        let mut any = false;
        for &p in f {
            if self.unmark_point(p) {
                any = true;
            }
        }
        any
    }

    /// Number of points with `EXTRUDE`.
    pub fn n_extruding(&self) -> usize {
        self.status.iter().filter(|&&s| s == ExtrudeMode::Extrude).count()
    }
}

// ---------------------------------------------------------------------------------
// setPointNumLayers and its parts
// ---------------------------------------------------------------------------------

/// `snappyLayerDriver::setNumLayers`: per point the maximum over its patches of the wanted
/// layers; returns `nAddedCells` (the ideal number of added cells).
pub fn set_num_layers(
    m: &MergeMesh,
    num_layers_per_patch: &[i64],
    patch_ids: &[usize],
    pp: &LayerPatch,
    st: &mut ExtrudeState,
) -> i64 {
    let starts = m.patch_starts();
    let n = pp.n_points();
    let point_of: HashMap<usize, usize> = pp.mesh_points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    let mut max_layers = vec![i64::MIN; n];
    let mut min_layers = vec![i64::MAX; n];
    for &patch in patch_ids {
        let wanted = num_layers_per_patch[patch];
        // polyPatch::meshPoints order does not matter for a max/min
        for f in starts[patch]..starts[patch + 1] {
            for &p in &m.mesh.faces[f] {
                let i = point_of[&p];
                max_layers[i] = max_layers[i].max(wanted);
                min_layers[i] = min_layers[i].min(wanted);
            }
        }
    }
    for i in 0..n {
        assert!(max_layers[i] != i64::MIN && min_layers[i] != i64::MAX, "patch point {i} in no layer patch");
        st.n_layers[i] = max_layers[i];
    }
    let mut n_added = 0i64;
    for f in &pp.local_faces {
        let mut n_cells = 0;
        for &p in f {
            n_cells = n_cells.max(st.n_layers[p]);
        }
        n_added += n_cells;
    }
    n_added
}

fn fc_index(n: usize, i: usize) -> usize {
    (i + 1) % n
}
fn rc_index(n: usize, i: usize) -> usize {
    (i + n - 1) % n
}
fn constrain_fp(sz: i64, fp: i64) -> usize {
    (if fp >= sz {
        0
    } else if fp < 0 {
        sz - 1
    } else {
        fp
    }) as usize
}

/// `snappyLayerDriver::checkCommonOrder(nCommon, curFace, nbFace)` (`primitiveMesh::checkCommonOrder`):
/// true when the shared points form one string.
pub fn check_common_order(n_common: usize, cur: &[usize], nb: &[usize]) -> bool {
    for fp in 0..cur.len() {
        if let Some(nbi) = nb.iter().position(|&x| x == cur[fp]) {
            let fp_plus1 = fc_index(cur.len(), fp);
            let fp_min1 = rc_index(cur.len(), fp);
            let nb_plus1 = fc_index(nb.len(), nbi);
            let nb_min1 = rc_index(nb.len(), nbi);
            let (mut cur_inc, mut nb_inc): (i64, i64);
            if nb[nb_plus1] == cur[fp_plus1] {
                cur_inc = 1;
                nb_inc = 1;
            } else if nb[nb_plus1] == cur[fp_min1] {
                cur_inc = -1;
                nb_inc = 1;
            } else if nb[nb_min1] == cur[fp_min1] {
                cur_inc = -1;
                nb_inc = -1;
            } else {
                cur_inc = 1;
                nb_inc = -1;
            }
            let mut cur_nb = nbi as i64;
            let mut cur_fp = fp as i64;
            loop {
                cur_fp = constrain_fp(cur.len() as i64, cur_fp + cur_inc) as i64;
                cur_nb = constrain_fp(nb.len() as i64, cur_nb + nb_inc) as i64;
                if cur[cur_fp as usize] != nb[cur_nb as usize] {
                    break;
                }
            }
            cur_inc = -cur_inc;
            nb_inc = -nb_inc;
            for _ in 0..n_common {
                cur_fp = constrain_fp(cur.len() as i64, cur_fp + cur_inc) as i64;
                cur_nb = constrain_fp(nb.len() as i64, cur_nb + nb_inc) as i64;
                if cur[cur_fp as usize] != nb[cur_nb as usize] {
                    return false;
                }
            }
            break;
        }
    }
    true
}

/// `handleNonStringConnected`: faces connected on non-consecutive vertices are not extruded.
/// Returns the number of faces pairs found not to be strings.
pub fn handle_non_string_connected(pp: &LayerPatch, st: &mut ExtrudeState) -> usize {
    let mut n_bad = 0;
    for facei in 0..pp.local_faces.len() {
        let mut n_common: BTreeMap<usize, usize> = BTreeMap::new();
        for &p in &pp.local_faces[facei] {
            for &nb in &pp.point_faces[p] {
                if facei < nb {
                    *n_common.entry(nb).or_insert(0) += 1;
                }
            }
        }
        for (&nb_face, &nc) in &n_common {
            let cur = &pp.local_faces[facei];
            let nbf = &pp.local_faces[nb_face];
            if nc >= 2 && nc != nbf.len() && nc != cur.len() && !check_common_order(nc, cur, nbf) {
                let (c, n) = (cur.clone(), nbf.clone());
                st.unmark_face(&c);
                st.unmark_face(&n);
                n_bad += 1;
            }
        }
    }
    n_bad
}

/// What `handleNonManifolds` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonManifoldCounts {
    /// Points on an edge with more than two patch faces ("multiply connected").
    pub n_non_manifold_points: usize,
    /// Baffle faces (all edges shared with the same one other face) that were unextruded.
    pub n_baffle_faces: usize,
}

/// `handleNonManifolds` (serial: `edgeGlobalFaces` equals `edgeFaces`).
pub fn handle_non_manifolds(pp: &LayerPatch, st: &mut ExtrudeState) -> NonManifoldCounts {
    let mut non_manifold = vec![false; pp.n_points()];
    for (ei, ef) in pp.edge_faces.iter().enumerate() {
        if ef.len() > 2 {
            non_manifold[pp.edges[ei][0]] = true;
            non_manifold[pp.edges[ei][1]] = true;
        }
    }
    let n_nm = non_manifold.iter().filter(|&&b| b).count();
    for (i, &b) in non_manifold.iter().enumerate() {
        if b {
            st.unmark_point(i);
        }
    }
    // faceEdges, in face order
    let mut edge_of: HashMap<(usize, usize), usize> = HashMap::new();
    for (ei, e) in pp.edges.iter().enumerate() {
        edge_of.insert((e[0].min(e[1]), e[0].max(e[1])), ei);
    }
    let mut n_baffle = 0;
    for facei in 0..pp.local_faces.len() {
        let f = &pp.local_faces[facei];
        let fe: Vec<usize> = (0..f.len())
            .map(|k| {
                let (a, b) = (f[k], f[(k + 1) % f.len()]);
                edge_of[&(a.min(b), a.max(b))]
            })
            .collect();
        let g0 = &pp.edge_faces[fe[0]];
        if g0.len() == 2 {
            let e0 = (g0[0].min(g0[1]), g0[0].max(g0[1]));
            let mut is_baffle = true;
            for &ei in &fe[1..] {
                let g = &pp.edge_faces[ei];
                if g.len() != 2 || (g[0].min(g[1]), g[0].max(g[1])) != e0 {
                    is_baffle = false;
                    break;
                }
            }
            if is_baffle {
                let fc = f.clone();
                if st.unmark_face(&fc) {
                    n_baffle += 1;
                }
            }
        }
    }
    NonManifoldCounts { n_non_manifold_points: n_nm, n_baffle_faces: n_baffle }
}

/// `nomalsCombine` with `point::max` (`VGREAT` in every component) as the null value.
fn normals_combine(x: &mut V, y: V) {
    let max = [VGREAT; 3];
    if y != max {
        if *x == max {
            *x = y;
        } else {
            let s = dot(*x, y);
            *x = scale(s, *x);
        }
    }
}

/// `handleFeatureAngle`: points on edges whose two face normals differ by more than
/// `min_angle_deg` are not extruded. Returns the number of feature edges.
/// `min_cos = cos(degToRad(minAngle))` is passed in so the caller chooses the libm route.
pub fn handle_feature_angle(pp: &LayerPatch, min_cos: f64, st: &mut ExtrudeState) -> usize {
    let mut n_feats = 0;
    if min_cos < 1.0 - SMALL && min_cos > -1.0 + SMALL {
        for (ei, ef) in pp.edge_faces.iter().enumerate() {
            let mut n = [VGREAT; 3];
            for &f in ef {
                normals_combine(&mut n, pp.face_normals[f]);
            }
            if n != [VGREAT; 3] {
                let cos = dot(n, pp.face_normals[ef[0]]);
                if cos < min_cos {
                    let e = pp.edges[ei];
                    st.unmark_point(e[0]);
                    st.unmark_point(e[1]);
                    n_feats += 1;
                }
            }
        }
    }
    n_feats
}

/// `handleWarpedFaces`: faces whose vertices stand off the plane by more than `face_ratio` of
/// the owner cell edge length are not extruded. Returns the number of faces unextruded.
#[allow(clippy::too_many_arguments)]
pub fn handle_warped_faces(
    m: &MergeMesh,
    pp: &LayerPatch,
    face_ratio: f64,
    relative_sizes: &[bool],
    edge0_len: f64,
    cell_level: &[usize],
    st: &mut ExtrudeState,
) -> usize {
    let points = &m.mesh.points;
    let (centres, _) = face_centres_areas(points, &m.mesh.faces);
    let mut n_warped = 0;
    for i in 0..pp.addressing.len() {
        let face_i = pp.addressing[i];
        let patch_i = m.which_patch(face_i).expect("layer patch face");
        let f = &m.mesh.faces[face_i];
        if relative_sizes[patch_i] && f.len() > 3 {
            let own_level = cell_level[m.mesh.owner[face_i]];
            let edge_len = edge0_len / (1u64 << own_level) as f64;
            let fc = centres[face_i];
            let fn_ = pp.face_normals[i];
            let v_proj: Vec<f64> = f.iter().map(|&p| dot(sub(points[p], fc), fn_)).collect();
            let min_val = v_proj.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_val = v_proj.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if (max_val - min_val) > face_ratio * edge_len {
                let lf = pp.local_faces[i].clone();
                if st.unmark_face(&lf) {
                    n_warped += 1;
                }
            }
        }
    }
    n_warped
}

/// `growNoExtrusion`: `NOEXTRUDE` spreads to the other points of every face that has one.
/// Returns the number of points grown.
pub fn grow_no_extrusion(pp: &LayerPatch, st: &mut ExtrudeState) -> usize {
    let mut grown = st.status.clone();
    let mut n_grown = 0;
    for f in &pp.local_faces {
        if f.iter().any(|&p| st.status[p] == ExtrudeMode::NoExtrude) {
            for &p in f {
                if st.status[p] == ExtrudeMode::Extrude && grown[p] != ExtrudeMode::NoExtrude {
                    grown[p] = ExtrudeMode::NoExtrude;
                    n_grown += 1;
                }
            }
        }
    }
    st.status = grown;
    for i in 0..st.status.len() {
        if st.status[i] == ExtrudeMode::NoExtrude {
            st.disp[i] = [0.0; 3];
            st.n_layers[i] = 0;
        }
    }
    n_grown
}

/// What `setPointNumLayers` found, for the log comparison.
#[derive(Debug, Clone)]
pub struct PointNumLayersReport {
    /// `nIdealTotAddedCells`.
    pub n_ideal_added_cells: i64,
    /// Face pairs `handleNonStringConnected` unextruded.
    pub n_non_string: usize,
    /// `handleNonManifolds` counts.
    pub non_manifold: NonManifoldCounts,
    /// Feature edges.
    pub n_feature_edges: usize,
    /// Warped faces (`None` when `handleWarpedFaces` did not run).
    pub n_warped: Option<usize>,
    /// Points added to the non-extruded set by `nGrow` passes, per pass.
    pub n_grown: Vec<usize>,
}

/// `snappyLayerDriver::setPointNumLayers`. `feature_cos` is `cos(degToRad(featureAngle))`.
#[allow(clippy::too_many_arguments)]
pub fn set_point_num_layers(
    m: &MergeMesh,
    params: &LayerParameters,
    patch_ids: &[usize],
    pp: &LayerPatch,
    feature_cos: f64,
    edge0_len: f64,
    cell_level: &[usize],
) -> (ExtrudeState, PointNumLayersReport) {
    let mut st = ExtrudeState::new(pp.n_points());
    let n_ideal = set_num_layers(m, &params.num_layers, patch_ids, pp, &mut st);
    let n_non_string = handle_non_string_connected(pp, &mut st);
    let non_manifold = handle_non_manifolds(pp, &mut st);
    let n_feature_edges = handle_feature_angle(pp, feature_cos, &mut st);
    // `!relativeSizes().found(false)`: all relative
    let n_warped = if !params.relative_sizes.contains(&false) {
        Some(handle_warped_faces(
            m,
            pp,
            params.dict.max_face_thickness_ratio,
            &params.relative_sizes,
            edge0_len,
            cell_level,
            &mut st,
        ))
    } else {
        None
    };
    let mut n_grown = Vec::new();
    for _ in 0..params.dict.n_grow {
        n_grown.push(grow_no_extrusion(pp, &mut st));
    }
    (st, PointNumLayersReport { n_ideal_added_cells: n_ideal, n_non_string, non_manifold, n_feature_edges, n_warped, n_grown })
}

// ---------------------------------------------------------------------------------
// calculateLayerThickness
// ---------------------------------------------------------------------------------

/// One row of the "patch faces layers avg thickness" table.
#[derive(Debug, Clone, PartialEq)]
pub struct ThicknessRow {
    /// Patch index.
    pub patch: usize,
    /// Faces in the patch.
    pub n_faces: usize,
    /// `numLayers()[patch]`.
    pub n_layers: i64,
    /// Average near-wall (first layer) thickness over the patch points.
    pub avg_near_wall: f64,
    /// Average overall thickness.
    pub avg_thickness: f64,
}

/// Per pp point the wanted overall thickness, minimum thickness and expansion ratio.
#[derive(Debug, Clone)]
pub struct LayerThickness {
    /// Overall thickness of all layers.
    pub thickness: Vec<f64>,
    /// Truncation thickness.
    pub min_thickness: Vec<f64>,
    /// Expansion ratio.
    pub expansion_ratio: Vec<f64>,
    /// The per-patch table.
    pub rows: Vec<ThicknessRow>,
}

/// `snappyLayerDriver::calculateLayerThickness`.
#[allow(clippy::too_many_arguments)]
pub fn calculate_layer_thickness(
    m: &MergeMesh,
    pp: &LayerPatch,
    patch_ids: &[usize],
    params: &LayerParameters,
    cell_level: &[usize],
    patch_n_layers: &[i64],
    edge0_len: f64,
) -> LayerThickness {
    let n = pp.n_points();
    let starts = m.patch_starts();
    let point_of: HashMap<usize, usize> = pp.mesh_points.iter().enumerate().map(|(i, &p)| (p, i)).collect();
    // patch meshPoints, in patch order of first appearance
    let patch_points = |patch: usize| -> Vec<usize> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for f in starts[patch]..starts[patch + 1] {
            for &p in &m.mesh.faces[f] {
                if seen.insert(p) {
                    out.push(p);
                }
            }
        }
        out
    };
    let mut first = vec![GREAT; n];
    let mut final_ = vec![GREAT; n];
    let mut total = vec![GREAT; n];
    let mut exp_ratio = vec![GREAT; n];
    let mut min_thickness = vec![GREAT; n];
    let mut thickness = vec![GREAT; n];
    let mut expansion_ratio = vec![GREAT; n];
    for &patch in patch_ids {
        for mp in patch_points(patch) {
            let i = point_of[&mp];
            let nb = params.numbers[patch];
            first[i] = first[i].min(nb.first);
            final_[i] = final_[i].min(nb.final_);
            total[i] = total[i].min(nb.total);
            exp_ratio[i] = exp_ratio[i].min(nb.expansion);
            min_thickness[i] = min_thickness[i].min(params.min_thickness[patch]);
        }
    }
    // maxPointLevel
    let mut max_point_level: Vec<i64> = vec![i64::MIN; n];
    for (i, f) in pp.local_faces.iter().enumerate() {
        let own_level = cell_level[m.mesh.owner[pp.addressing[i]]] as i64;
        for &p in f {
            max_point_level[p] = max_point_level[p].max(own_level);
        }
    }
    let mut edge_len = vec![GREAT; n];
    let mut spec = vec![ThicknessModel::FirstAndTotal; n];
    let mut is_relative = vec![false; n];
    for &patch in patch_ids {
        let patch_spec = params.layer_models[patch];
        let rel = params.relative_sizes[patch];
        for mp in patch_points(patch) {
            let i = point_of[&mp];
            edge_len[i] = edge_len[i].min(edge0_len / (1u64 << max_point_level[i]) as f64);
            spec[i] = spec[i].max(patch_spec);
            is_relative[i] = is_relative[i] || rel;
        }
    }
    for i in 0..n {
        let point_spec = spec[i];
        if point_spec == ThicknessModel::FirstAndRelativeFinal {
            final_[i] *= edge_len[i];
            if is_relative[i] {
                total[i] *= edge_len[i];
                min_thickness[i] *= edge_len[i];
            }
        } else if is_relative[i] {
            first[i] *= edge_len[i];
            final_[i] *= edge_len[i];
            total[i] *= edge_len[i];
            min_thickness[i] *= edge_len[i];
        }
        let nb = LayerNumbers { first: first[i], final_: final_[i], total: total[i], expansion: exp_ratio[i] };
        thickness[i] = thickness[i].min(layer_thickness_spec(point_spec, patch_n_layers[i], nb));
        expansion_ratio[i] = expansion_ratio[i].min(layer_expansion_ratio_spec(point_spec, patch_n_layers[i], nb));
    }
    let mut rows = Vec::new();
    for &patch in patch_ids {
        let spec = params.layer_models[patch];
        let mut sum_thickness = 0.0;
        let mut sum_near_wall = 0.0;
        let mut n_master = 0usize;
        for mp in patch_points(patch) {
            let i = point_of[&mp];
            sum_thickness += thickness[i];
            let nb = LayerNumbers { first: first[i], final_: final_[i], total: thickness[i], expansion: expansion_ratio[i] };
            sum_near_wall += first_layer_thickness_spec(spec, patch_n_layers[i], nb);
            n_master += 1;
        }
        let (avg_t, avg_nw) = if n_master > 0 {
            (sum_thickness / n_master as f64, sum_near_wall / n_master as f64)
        } else {
            (0.0, 0.0)
        };
        rows.push(ThicknessRow {
            patch,
            n_faces: starts[patch + 1] - starts[patch],
            n_layers: params.num_layers[patch],
            avg_near_wall: avg_nw,
            avg_thickness: avg_t,
        });
    }
    LayerThickness { thickness, min_thickness, expansion_ratio, rows }
}

/// `avgPointData(pp, pointData)`: per patch face the average of its points' data.
pub fn avg_point_data(pp: &LayerPatch, point_data: &[f64]) -> Vec<f64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `finalLayerThicknessRatio`: one layer is 1, equal ratio is 1/n, geometric otherwise.
    #[test]
    fn final_ratio_cases() {
        assert_eq!(final_layer_thickness_ratio(1, 1.0), 1.0);
        assert_eq!(final_layer_thickness_ratio(4, 1.0), 0.25);
        assert_eq!(final_layer_thickness_ratio(0, 1.5), 0.0);
        // r^(n-1) (1-r)/(1-r^n) for n=3, r=2: 4 * (-1) / (-7)
        assert!((final_layer_thickness_ratio(3, 2.0) - 4.0 / 7.0).abs() < 1e-14);
    }

    /// The four thickness numbers of `finalAndExpansion`: first layer, overall, and the slices
    /// of `layerThickness(nLayers, thickness, ratio, start, size)` add up to the overall.
    #[test]
    fn final_and_expansion_slices_sum_to_total() {
        let mut n = LayerNumbers { first: -123.0, final_: 0.3, total: -123.0, expansion: 1.2 };
        calculate_layer_parameters(ThicknessModel::FinalAndExpansion, 4, &mut n);
        let t = layer_thickness_spec(ThicknessModel::FinalAndExpansion, 4, n);
        assert_eq!(n.total, t);
        assert!((n.first - 0.3 / 1.2f64.powi(3)).abs() < 1e-14);
        let a = layer_thickness_slice(4, t, 1.2, 0, 2);
        let b = layer_thickness_slice(4, t, 1.2, 2, 2);
        assert!((a + b - t).abs() < 1e-13);
        assert_eq!(layer_thickness_slice(4, t, 1.2, 0, 0), 0.0);
        // equal ratio
        assert!((layer_thickness_slice(4, 1.0, 1.0, 0, 1) - 0.25).abs() < 1e-15);
    }

    /// `firstAndOverall` -> expansion ratio by Newton: a 3-layer stack with first 1 and total 7
    /// has ratio 2.
    #[test]
    fn newton_expansion_ratio() {
        let r = layer_expansion_ratio_newton(3, 7.0);
        assert!((r - 2.0).abs() < 1e-7, "{r}");
        assert_eq!(layer_expansion_ratio_newton(1, 5.0), 1.0);
        assert_eq!(layer_expansion_ratio_newton(4, 4.0), 1.0);
        let mut n = LayerNumbers { first: 1.0, final_: -123.0, total: 7.0, expansion: -123.0 };
        calculate_layer_parameters(ThicknessModel::FirstAndTotal, 3, &mut n);
        assert!((n.final_ - 4.0).abs() < 1e-6);
        let mut n = LayerNumbers { first: 1.0, final_: 4.0, total: -123.0, expansion: -123.0 };
        calculate_layer_parameters(ThicknessModel::FirstAndRelativeFinal, 3, &mut n);
        assert!((n.expansion - 2.0).abs() < 1e-12 && (n.total - 7.0).abs() < 1e-12);
        let mut n = LayerNumbers { first: -123.0, final_: -123.0, total: 7.0, expansion: 2.0 };
        calculate_layer_parameters(ThicknessModel::TotalAndExpansion, 3, &mut n);
        // ESI quirk, ported as read: `calculateLayerParameters` passes `VGREAT` as the total
        // thickness to `firstLayerThickness` here, so the stored first thickness is ~1e300
        // (the printed table recomputes it from the real thickness).
        assert!(n.first > 1e299);
        assert!((n.final_ - 4.0).abs() < 1e-12);
        let first = first_layer_thickness_spec(ThicknessModel::TotalAndExpansion, 3, LayerNumbers { first: 0.0, final_: 0.0, total: 7.0, expansion: 2.0 });
        assert!((first - 1.0).abs() < 1e-12);
        let mut n = LayerNumbers { first: -123.0, final_: 4.0, total: 7.0, expansion: -123.0 };
        calculate_layer_parameters(ThicknessModel::FinalAndTotal, 3, &mut n);
        assert!((n.first - 1.0).abs() < 1e-5 && (n.expansion - 2.0).abs() < 1e-5, "{n:?}");
        let mut n = LayerNumbers { first: 1.0, final_: -123.0, total: -123.0, expansion: 2.0 };
        calculate_layer_parameters(ThicknessModel::FirstAndExpansion, 3, &mut n);
        assert!((n.total - 7.0).abs() < 1e-12 && (n.final_ - 4.0).abs() < 1e-12);
    }

    /// Two unit squares side by side: unextrude / grow / check-common-order behave as ESI's.
    #[test]
    fn check_common_order_detects_split_strings() {
        // faces share points 1 and 4 (adjacent in both): a string
        assert!(check_common_order(2, &[0, 1, 4, 3], &[1, 2, 5, 4]));
        // faces share points 0 and 2 only (diagonal): not a string
        assert!(!check_common_order(2, &[0, 1, 2, 3], &[0, 4, 2, 5]));
    }

    fn two_quads() -> (MergeMesh, LayerPatch) {
        use super::super::esi_snap::SnapMesh;
        // one cell below a 2x1 strip of two boundary faces; patch 0 = both faces
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        let faces = vec![vec![0, 3, 4, 1], vec![1, 4, 5, 2]];
        let mesh = SnapMesh { points, faces, owner: vec![0, 1], neighbour: vec![] };
        let m = MergeMesh::from_patch_starts(mesh, &[0, 2]);
        let pp = LayerPatch::new(&m, &[0]);
        (m, pp)
    }

    #[test]
    fn layer_patch_addressing_and_manifold_counts() {
        let (m, pp) = two_quads();
        assert_eq!(pp.n_points(), 6);
        assert_eq!(pp.edges.len(), 7);
        assert_eq!(pp.edge_faces.iter().filter(|e| e.len() == 2).count(), 1);
        let mut st = ExtrudeState::new(6);
        let nm = handle_non_manifolds(&pp, &mut st);
        assert_eq!(nm, NonManifoldCounts { n_non_manifold_points: 0, n_baffle_faces: 0 });
        // flat strip: no feature edges at 30 degrees
        let cos30 = (30.0f64 * std::f64::consts::PI / 180.0).cos();
        assert_eq!(handle_feature_angle(&pp, cos30, &mut st), 0);
        let ncl = set_num_layers(&m, &[2], &[0], &pp, &mut st);
        assert_eq!(ncl, 4);
        assert_eq!(st.n_layers, vec![2; 6]);
        assert_eq!(handle_non_string_connected(&pp, &mut st), 0);
        // unextrude one point and grow: its face neighbours go too
        st.unmark_point(0);
        assert_eq!(grow_no_extrusion(&pp, &mut st), 3);
        assert_eq!(st.n_extruding(), 2);
        assert!(st.disp[3] == [0.0; 3] && st.n_layers[3] == 0);
    }

    /// A folded strip has a feature edge at the fold: its two points lose their extrusion.
    #[test]
    fn feature_angle_marks_fold() {
        use super::super::esi_snap::SnapMesh;
        let points = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 1.0],
        ];
        let faces = vec![vec![0, 3, 4, 1], vec![1, 4, 5, 2]];
        let mesh = SnapMesh { points, faces, owner: vec![0, 1], neighbour: vec![] };
        let m = MergeMesh::from_patch_starts(mesh, &[0, 2]);
        let pp = LayerPatch::new(&m, &[0]);
        let mut st = ExtrudeState::new(6);
        let cos30 = (30.0f64 * std::f64::consts::PI / 180.0).cos();
        assert_eq!(handle_feature_angle(&pp, cos30, &mut st), 1);
        assert_eq!(st.n_extruding(), 4);
    }

    /// `calculateLayerThickness` on the strip: relative `finalAndExpansion`, 1 layer, level 1,
    /// edge0 = 4: thickness 0.3 * 2 per point.
    #[test]
    fn calculate_thickness_relative() {
        let (m, pp) = two_quads();
        let dict = LayerDict {
            relative_sizes: true,
            numbers: LayerNumbers { first: -123.0, final_: 0.3, total: -123.0, expansion: 1.0 },
            model: ThicknessModel::FinalAndExpansion,
            min_thickness: 0.25,
            feature_angle: 30.0,
            n_grow: 0,
            max_face_thickness_ratio: 0.5,
            n_buffer_cells_no_extrude: 0,
            n_layer_iter: 50,
            n_relaxed_iter: 20,
            n_outer_iter: 1,
        };
        let params = LayerParameters::new(&dict, 1, &[(0, PatchLayerDict { n_surface_layers: 1, ..Default::default() })]);
        let mut st = ExtrudeState::new(6);
        set_num_layers(&m, &params.num_layers, &[0], &pp, &mut st);
        let warped = handle_warped_faces(&m, &pp, 0.5, &params.relative_sizes, 4.0, &[1, 1], &mut st);
        assert_eq!(warped, 0);
        let lt = calculate_layer_thickness(&m, &pp, &[0], &params, &[1, 1], &st.n_layers, 4.0);
        assert!(lt.thickness.iter().all(|&t| (t - 0.6).abs() < 1e-15));
        assert!(lt.min_thickness.iter().all(|&t| (t - 0.5).abs() < 1e-15));
        assert_eq!(lt.rows.len(), 1);
        assert!((lt.rows[0].avg_near_wall - 0.6).abs() < 1e-15);
        let avg = avg_point_data(&pp, &lt.thickness);
        assert!(avg.iter().all(|&a| (a - 0.6).abs() < 1e-15));
    }
}
