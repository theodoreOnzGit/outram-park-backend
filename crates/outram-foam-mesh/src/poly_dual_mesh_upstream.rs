// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org) -- the `polyDualMesh` utility, its
// `Foam::meshDualiser` engine and the supporting primitiveMesh / polyTopoChange
// bookkeeping:
//   applications/utilities/mesh/manipulation/polyDualMesh/{polyDualMesh.C,
//     meshDualiser.C, meshDualiser.H},
//   src/meshTools/edgeFaceCirculator/, src/polyTopoChange/polyTopoChange/
//     polyTopoChange.C (getFaceOrder, compact),
//   src/OpenFOAM/meshes/primitiveMesh/ (primitiveMeshEdges.C,
//     primitiveMeshFaceCentresAndAreas.C, primitiveMeshCellCentresAndVols.C),
//   src/OpenFOAM/meshes/primitiveMesh/PrimitivePatch/ (calcAddressing,
//     checkPointManifold), src/OpenFOAM/meshes/meshShapes/face/faceTemplates.C,
//   src/OpenFOAM/containers/HashTables/ (labelHashSet iteration order).
// OpenFOAM-dev commit 852527377c7e87e33d51b19ca8974b5db60af28b.
// Also compared with OpenFOAM.com v2506 (applications/utilities/mesh/
// manipulation/polyDualMesh, primitiveMeshTools.C).
// Copyright (C) 2011-2026 OpenFOAM Foundation
// Copyright (C) 2016-2025 OpenCFD Ltd.
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

//! `polyDualMesh` in **upstream mode** (gh:#1472, gh:#1471, gh:#1473): a
//! line-for-line port of OpenFOAM's `polyDualMesh` utility driver
//! (`simpleMarkFeatures`) and its `meshDualiser` engine, with `polyTopoChange`'s
//! point / cell / face numbering, so the output can be compared with an
//! upstream run file by file.
//!
//! This is **beside** the surface-preserving default of
//! [`poly_dual_mesh`](crate::poly_dual_mesh); nothing here changes that
//! default. Choose it explicitly with [`poly_dual_mesh_upstream`].
//!
//! ## What differs from the default (`poly_dual_mesh_with`)
//!
//! | | default (surface-preserving) | upstream mode (this file) |
//! |---|---|---|
//! | dual points | cell centres, every boundary-face centre, **every** boundary-edge midpoint, feature points | cell centres, boundary-face centres, **feature**-edge midpoints (and midpoints of boundary edges at feature points), feature points; more only where dual cells differ |
//! | dual cells | one per primal point, numbered as the primal point | feature points first (hash-set order), multi-cell points, then the other points; numbered as `polyTopoChange` adds them |
//! | options | `featureAngle` | `featureAngle`, `splitAllFaces`, `concaveMultiCells`, face-zone preservation |
//! | face / cell centres | the crate's own fan formulas | [`CentreFormula::Dev`] (OpenFOAM-dev `face::areaAndCentre`, area weights projected on the normal) or [`CentreFormula::Esi2506`] (`mag(n)` weights) |
//! | face order | internal faces then patches, in construction order | `polyTopoChange::getFaceOrder`: upper-triangular by dual cell, then patches |
//!
//! ## Hash-set order
//!
//! `simpleMarkFeatures` returns its feature sets with `labelHashSet::toc()`,
//! so the numbering of dual points and cells follows the hash table's bucket
//! order. [`FoamLabelHashSet`] reproduces OpenFOAM-dev's table (identity hash,
//! power-of-two table, insertion at the bucket head, doubling above load 0.8,
//! re-insertion in iteration order). It is checked against the OpenFOAM.com
//! v2506 binary in the V&V record, not assumed.
//!
//! ## Honest limits
//!
//! - Face zones are supplied by the caller ([`FaceZoneInput`]); the crate's I/O
//!   [`PolyMesh`](crate::poly_dual_mesh::PolyMesh) has no zones.
//! - Parallel (processor) patches and cyclic couplings are not handled.
//! - OpenFOAM.com's `polyDualMesh` also carries fields and writes the face
//!   zones of the dual; neither is done here.

use std::collections::HashMap;

use petir::mathf::RealMath;

use outram_foam_basic_lib::primitives::Vector3;

use crate::poly_dual_mesh::{DualMesh, PolyMesh};
use crate::MeshError;

type V = [f64; 3];

// ───────────────────────────── vector helpers ───────────────────────────────
// Explicit component arithmetic, in OpenFOAM's operation order (no fused
// multiply-add: Rust never contracts, and the upstream build is -O3 without
// -march=native).

fn vsub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vadd(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn vscale(s: f64, a: V) -> V {
    [s * a[0], s * a[1], s * a[2]]
}
fn vdiv(a: V, s: f64) -> V {
    [a[0] / s, a[1] / s, a[2] / s]
}
fn vcross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vdot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vmag(a: V) -> f64 {
    vdot(a, a).sqrt()
}
/// OpenFOAM-dev `normalised`: `magV > 0 ? v/magV : Zero`.
fn vnormalised(a: V) -> V {
    let m = vmag(a);
    if m > 0.0 {
        vdiv(a, m)
    } else {
        [0.0; 3]
    }
}
fn to_v(p: &Vector3) -> V {
    [p.x, p.y, p.z]
}
fn from_v(p: V) -> Vector3 {
    Vector3::new(p[0], p[1], p[2])
}

const V_SMALL: f64 = 1.0e-300;
const ROOT_V_SMALL: f64 = 1.0e-150;
const SMALL: f64 = 1.0e-15;

// ───────────────────────── face / cell centres (gh:#1473) ───────────────────

/// Which OpenFOAM line's face-centre routine to use.
///
/// The two lines differ for faces with more than three vertices (gh:#1473):
/// OpenFOAM-dev weights the point-average triangles by their area projected
/// on the face normal; OpenFOAM.com v2506 weights by `mag(n)`. For a planar
/// face the two agree only to round-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CentreFormula {
    /// OpenFOAM-dev `face::areaAndCentre` (faceTemplates.C:79-150), the line
    /// the port cites (`852527377c7e`).
    #[default]
    Dev,
    /// OpenFOAM.com v2506 `primitiveMeshTools::makeFaceCentresAndAreas`.
    Esi2506,
}

/// Area vector and centre of one face, exactly as the chosen OpenFOAM line
/// computes them (operation order included).
pub fn face_area_and_centre(formula: CentreFormula, ps: &[Vector3]) -> (Vector3, Vector3) {
    let p: Vec<V> = ps.iter().map(to_v).collect();
    let (a, c) = area_and_centre_v(formula, &p);
    (from_v(a), from_v(c))
}

fn area_and_centre_v(formula: CentreFormula, ps: &[V]) -> (V, V) {
    let n = ps.len();
    if n == 3 {
        let area = vscale(0.5, vcross(vsub(ps[1], ps[0]), vsub(ps[2], ps[0])));
        let centre = vscale(1.0 / 3.0, vadd(vadd(ps[0], ps[1]), ps[2]));
        return (area, centre);
    }
    let mut p_avg = ps[0];
    for p in &ps[1..] {
        p_avg = vadd(p_avg, *p);
    }
    let p_avg = vdiv(p_avg, n as f64);
    match formula {
        CentreFormula::Dev => {
            let mut sum_a = [0.0; 3];
            for i in 0..n {
                let (p, pn) = (ps[i], ps[(i + 1) % n]);
                sum_a = vadd(sum_a, vcross(vsub(pn, p), vsub(p_avg, p)));
            }
            let sum_a_hat = vnormalised(sum_a);
            let mut sum_an = 0.0;
            let mut sum_anc = [0.0; 3];
            for i in 0..n {
                let (p, pn) = (ps[i], ps[(i + 1) % n]);
                let a = vcross(vsub(pn, p), vsub(p_avg, p));
                let c = vadd(vadd(p, pn), p_avg);
                let an = vdot(a, sum_a_hat);
                sum_an += an;
                sum_anc = vadd(sum_anc, vscale(an, c));
            }
            let centre = if sum_an > V_SMALL {
                vdiv(vscale(1.0 / 3.0, sum_anc), sum_an)
            } else {
                p_avg
            };
            (vscale(1.0 / 2.0, sum_a), centre)
        }
        CentreFormula::Esi2506 => {
            let mut sum_n = [0.0; 3];
            let mut sum_a = 0.0;
            let mut sum_ac = [0.0; 3];
            for i in 0..n {
                let (this, next) = (ps[i], ps[(i + 1) % n]);
                let c = vadd(vadd(this, next), p_avg);
                let nn = vcross(vsub(next, this), vsub(p_avg, this));
                let a = vmag(nn);
                sum_n = vadd(sum_n, nn);
                sum_a += a;
                sum_ac = vadd(sum_ac, vscale(a, c));
            }
            if sum_a < ROOT_V_SMALL {
                (p_avg_zero(), p_avg)
            } else {
                (vscale(0.5, sum_n), vdiv(vscale(1.0 / 3.0, sum_ac), sum_a))
            }
        }
    }
}

fn p_avg_zero() -> V {
    [0.0; 3]
}

/// Cell centres `primitiveMesh::makeCellCentresAndVols` (identical in
/// OpenFOAM-dev and v2506): pyramids about the mean face centre,
/// `pc = 3/4 fc + 1/4 cEst`. Returns `(centres, volumes)`.
pub fn cell_centres_and_volumes(
    n_cells: usize,
    owner: &[usize],
    neighbour: &[usize],
    f_ctrs: &[Vector3],
    f_areas: &[Vector3],
) -> (Vec<Vector3>, Vec<f64>) {
    let fc: Vec<V> = f_ctrs.iter().map(to_v).collect();
    let fa: Vec<V> = f_areas.iter().map(to_v).collect();
    let mut c_est = vec![[0.0; 3]; n_cells];
    let mut n_cell_faces = vec![0usize; n_cells];
    for (f, &o) in owner.iter().enumerate() {
        c_est[o] = vadd(c_est[o], fc[f]);
        n_cell_faces[o] += 1;
    }
    for (f, &n) in neighbour.iter().enumerate() {
        c_est[n] = vadd(c_est[n], fc[f]);
        n_cell_faces[n] += 1;
    }
    for c in 0..n_cells {
        c_est[c] = vdiv(c_est[c], n_cell_faces[c] as f64);
    }
    let mut ctrs = vec![[0.0; 3]; n_cells];
    let mut vols = vec![0.0; n_cells];
    for (f, &o) in owner.iter().enumerate() {
        let pyr3 = vdot(fa[f], vsub(fc[f], c_est[o]));
        let pc = vadd(vscale(3.0 / 4.0, fc[f]), vscale(1.0 / 4.0, c_est[o]));
        ctrs[o] = vadd(ctrs[o], vscale(pyr3, pc));
        vols[o] += pyr3;
    }
    for (f, &n) in neighbour.iter().enumerate() {
        let pyr3 = vdot(fa[f], vsub(c_est[n], fc[f]));
        let pc = vadd(vscale(3.0 / 4.0, fc[f]), vscale(1.0 / 4.0, c_est[n]));
        ctrs[n] = vadd(ctrs[n], vscale(pyr3, pc));
        vols[n] += pyr3;
    }
    for c in 0..n_cells {
        if vols[c].abs() > V_SMALL {
            ctrs[c] = vdiv(ctrs[c], vols[c]);
        } else {
            ctrs[c] = c_est[c];
        }
    }
    for v in vols.iter_mut() {
        *v *= 1.0 / 3.0;
    }
    (ctrs.into_iter().map(from_v).collect(), vols)
}

// ───────────────────────────── labelHashSet ─────────────────────────────────

/// OpenFOAM-dev `labelHashSet`, reproduced for its **iteration order** only.
///
/// `HashTable<nil, label, Hash<label>>`: `Hash<label>` is the identity, the
/// table size is a power of two (`canonicalSize`), a new key goes to the head
/// of its bucket, the table doubles when `nElmts/tableSize > 0.8`, a resize
/// re-inserts keys in the old iteration order, and `toc()` walks the buckets
/// 0.. and each chain head to tail. `HashSet(size = 128)` by default.
#[derive(Debug, Clone)]
pub struct FoamLabelHashSet {
    buckets: Vec<Vec<usize>>,
    n_elmts: usize,
}

fn canonical_size(size: usize) -> usize {
    if size < 1 {
        return 0;
    }
    let mut good = 1usize;
    while good < size {
        good <<= 1;
    }
    good
}

impl FoamLabelHashSet {
    /// A set constructed with `size` (`HashSet(const label size = 128)`).
    pub fn with_size(size: usize) -> Self {
        Self {
            buckets: vec![Vec::new(); canonical_size(size)],
            n_elmts: 0,
        }
    }

    /// OpenFOAM.com v2506's `reserve(n)` on an empty set: capacity
    /// `canonicalSize(2 n)` (initial load factor 0.5; `canonicalSize` there
    /// has a lower limit of 8 buckets). Dev's `HashSet(n)` gives
    /// `canonicalSize(n)` instead, so the same insertions can come back in a
    /// different order.
    pub fn with_reserve_esi2506(n: usize) -> Self {
        if n == 0 {
            return Self::with_size(0);
        }
        Self::with_size((2 * n).max(8))
    }

    /// The default-constructed set (`size = 128`).
    pub fn new() -> Self {
        Self::with_size(128)
    }

    /// Insert `key`; `false` if present (`HashTable::set`, protect = true).
    pub fn insert(&mut self, key: usize) -> bool {
        if self.buckets.is_empty() {
            self.resize(2);
        }
        let size = self.buckets.len();
        let idx = key & (size - 1);
        if self.buckets[idx].contains(&key) {
            return false;
        }
        self.buckets[idx].insert(0, key);
        self.n_elmts += 1;
        if self.n_elmts as f64 / size as f64 > 0.8 {
            self.resize(2 * size);
        }
        true
    }

    fn resize(&mut self, sz: usize) {
        let new_size = canonical_size(sz);
        if new_size == self.buckets.len() {
            return;
        }
        let keys = self.toc();
        let mut tmp = FoamLabelHashSet::with_size(new_size);
        for k in keys {
            tmp.insert(k);
        }
        self.buckets = tmp.buckets;
    }

    /// Remove `key` (the chain order of the others is kept).
    pub fn erase(&mut self, key: usize) -> bool {
        if self.buckets.is_empty() {
            return false;
        }
        let idx = key & (self.buckets.len() - 1);
        if let Some(pos) = self.buckets[idx].iter().position(|&k| k == key) {
            self.buckets[idx].remove(pos);
            self.n_elmts -= 1;
            true
        } else {
            false
        }
    }

    /// Whether `key` is in the set.
    pub fn found(&self, key: usize) -> bool {
        !self.buckets.is_empty() && self.buckets[key & (self.buckets.len() - 1)].contains(&key)
    }

    /// `toc()`: the keys in bucket order.
    pub fn toc(&self) -> Vec<usize> {
        self.buckets.iter().flatten().copied().collect()
    }

    /// Number of keys.
    pub fn len(&self) -> usize {
        self.n_elmts
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.n_elmts == 0
    }
}

impl Default for FoamLabelHashSet {
    fn default() -> Self {
        Self::new()
    }
}

// ───────────────────────────── options ──────────────────────────────────────

/// A face zone of the primal mesh, as `simpleMarkFeatures` reads it: only the
/// member faces matter (flip maps do not affect the dual's connectivity in
/// OpenFOAM-dev, which clears the zones before `meshDualiser`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceZoneInput {
    /// Zone name.
    pub name: String,
    /// Member face indices into the primal face list.
    pub faces: Vec<usize>,
}

/// Options of upstream-mode `polyDualMesh` (the utility's command line).
#[derive(Debug, Clone)]
pub struct UpstreamOptions {
    /// `featureAngle` in degrees `[0, 180]`.
    pub feature_angle_deg: f64,
    /// `-splitAllFaces`: one dual face per primal cell an edge passes through.
    pub split_all_faces: bool,
    /// `-concaveMultiCells`: points on concave feature edges become several
    /// cells.
    pub concave_multi_cells: bool,
    /// Preserve face zones (default); `-doNotPreserveFaceZones` clears it.
    pub preserve_face_zones: bool,
    /// The primal mesh's face zones.
    pub face_zones: Vec<FaceZoneInput>,
    /// Face-centre formula; see [`CentreFormula`].
    pub centres: CentreFormula,
    /// OpenFOAM.com v2506 sizes the feature-face hash set with
    /// `reserve(n)` (capacity `2n`), OpenFOAM-dev with `labelHashSet(n)`
    /// (capacity `n`); the capacity fixes the `toc()` order of that set and
    /// so the numbering of the face-centre dual points.
    pub esi2506_hash_reserve: bool,
}

impl UpstreamOptions {
    /// Defaults of the utility: no `-splitAllFaces`, no `-concaveMultiCells`,
    /// face zones preserved, OpenFOAM-dev centres.
    pub fn new(feature_angle_deg: f64) -> Self {
        Self {
            feature_angle_deg,
            split_all_faces: false,
            concave_multi_cells: false,
            preserve_face_zones: true,
            face_zones: Vec::new(),
            centres: CentreFormula::Dev,
            esi2506_hash_reserve: false,
        }
    }
    /// Everything that differs between the two OpenFOAM lines, set to
    /// OpenFOAM.com v2506: its face-centre formula and its hash-set sizing.
    pub fn with_esi2506_flavour(mut self) -> Self {
        self.centres = CentreFormula::Esi2506;
        self.esi2506_hash_reserve = true;
        self
    }
    /// Enable `-splitAllFaces`.
    pub fn with_split_all_faces(mut self) -> Self {
        self.split_all_faces = true;
        self
    }
    /// Enable `-concaveMultiCells`.
    pub fn with_concave_multi_cells(mut self) -> Self {
        self.concave_multi_cells = true;
        self
    }
    /// `-doNotPreserveFaceZones`.
    pub fn without_face_zone_preservation(mut self) -> Self {
        self.preserve_face_zones = false;
        self
    }
    /// Supply the primal mesh's face zones.
    pub fn with_face_zones(mut self, zones: Vec<FaceZoneInput>) -> Self {
        self.face_zones = zones;
        self
    }
    /// Choose the face-centre formula.
    pub fn with_centre_formula(mut self, f: CentreFormula) -> Self {
        self.centres = f;
        self
    }
}

// ───────────────────────────── primal topology ──────────────────────────────

struct Topo<'a> {
    m: &'a PolyMesh,
    nf: usize,
    /// Mesh edges in `primitiveMesh::calcEdges` order for an unordered
    /// polyMesh: lexicographic `(min, max)`.
    edges: Vec<(usize, usize)>,
    edge_id: HashMap<(usize, usize), usize>,
    /// `faceEdges[f][fp]`: the edge between `f[fp]` and `f[fp + 1]`.
    face_edges: Vec<Vec<usize>>,
    /// Ascending face lists.
    edge_faces: Vec<Vec<usize>>,
    /// Ascending edge lists.
    point_edges: Vec<Vec<usize>>,
    /// Faces owned first (ascending), then faces neighboured (ascending).
    cell_faces: Vec<Vec<usize>>,
    /// Ascending cell lists.
    point_cells: Vec<Vec<usize>>,
    is_boundary_edge: Vec<bool>,
}

fn ekey(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// `primitiveMesh::calcPointOrder`'s verdict as `reset` stores it: the number
/// of internal points when the points are ordered (internal points first),
/// else `None` (`nInternalPoints_ = -1`).
fn n_internal_points_if_ordered(m: &PolyMesh) -> Option<usize> {
    let np = m.points.len();
    let mut old_to_new = vec![usize::MAX; np];
    let mut n_boundary = 0usize;
    for f in m.n_internal_faces..m.faces.len() {
        for &p in &m.faces[f] {
            if old_to_new[p] == usize::MAX {
                old_to_new[p] = n_boundary;
                n_boundary += 1;
            }
        }
    }
    let n_internal = np - n_boundary;
    let mut ordered = true;
    for f in 0..m.n_internal_faces {
        for &p in &m.faces[f] {
            if old_to_new[p] == usize::MAX {
                if p >= n_internal {
                    ordered = false;
                }
                old_to_new[p] = 0; // marked; the value is not needed here
            }
        }
    }
    ordered.then_some(n_internal)
}

/// `primitiveMesh::calcEdges` (primitiveMeshEdges.C): the mesh edges in
/// OpenFOAM's numbering. With ordered points (the usual case for a polyMesh
/// read from disk, `nInternalPoints_ != -1`) edges are grouped as internal to
/// internal points, internal to boundary, boundary to boundary, then the
/// edges of boundary faces last; otherwise all edges are upper-triangular
/// (lexicographic by `(min, max)`). Exact in both cases because it
/// reproduces `getEdge`'s creation order and the final `oldToNew`.
fn calc_edges(m: &PolyMesh) -> Vec<(usize, usize)> {
    /// `primitiveMesh::getEdge`: the edge between `p` and `q`, created (and
    /// registered in `pointEdges`) if absent. Returns `(id, created)`.
    fn get_edge(
        pe: &mut [Vec<usize>],
        es: &mut Vec<(usize, usize)>,
        p: usize,
        q: usize,
    ) -> (usize, bool) {
        for &e in &pe[p] {
            let (a, b) = es[e];
            if a == q || b == q {
                return (e, false);
            }
        }
        let id = es.len();
        pe[p].push(id);
        if q != p {
            pe[q].push(id);
        }
        es.push(ekey(p, q));
        (id, true)
    }

    let np = m.points.len();
    let n_int_pts = n_internal_points_if_ordered(m);
    let mut pe: Vec<Vec<usize>> = vec![Vec::new(); np];
    let mut es: Vec<(usize, usize)> = Vec::new();
    let mut n_ext = 0usize;
    let mut n_int0 = 0usize;
    let mut n_int1 = 0usize;
    match n_int_pts {
        None => {
            for f in &m.faces {
                let n = f.len();
                for fp in 0..n {
                    get_edge(&mut pe, &mut es, f[fp], f[(fp + 1) % n]);
                }
            }
        }
        Some(ni) => {
            for fi in m.n_internal_faces..m.faces.len() {
                let f = &m.faces[fi];
                let n = f.len();
                for fp in 0..n {
                    let (_, created) = get_edge(&mut pe, &mut es, f[fp], f[(fp + 1) % n]);
                    if created {
                        n_ext += 1;
                    }
                }
            }
            for fi in 0..m.n_internal_faces {
                let f = &m.faces[fi];
                let n = f.len();
                for fp in 0..n {
                    let (p, q) = (f[fp], f[(fp + 1) % n]);
                    let (_, created) = get_edge(&mut pe, &mut es, p, q);
                    if created {
                        if p < ni && q < ni {
                            n_int0 += 1;
                        } else if p < ni || q < ni {
                            n_int1 += 1;
                        }
                    }
                }
            }
        }
    }
    let n_edges = es.len();
    let (n_internal_edges, n_internal1_edges, n_internal0_edges) = match n_int_pts {
        None => (0, 0, n_edges),
        Some(_) => (n_edges - n_ext, n_int0 + n_int1, n_int0),
    };
    let mut old_to_new = vec![usize::MAX; n_edges];
    let mut internal0 = 0usize;
    let mut internal1 = n_internal0_edges;
    let mut internal2 = n_internal1_edges;
    let mut external = n_internal_edges;
    for pointi in 0..np {
        let mut nbr: Vec<(i64, usize)> = pe[pointi]
            .iter()
            .map(|&e| {
                let (a, b) = es[e];
                let other = if a == pointi { b } else { a };
                (if other < pointi { -1 } else { other as i64 }, e)
            })
            .collect();
        nbr.sort_by_key(|&(v, _)| v);
        for (v, e) in nbr {
            if v == -1 {
                continue;
            }
            match n_int_pts {
                None => {
                    old_to_new[e] = internal0;
                    internal0 += 1;
                }
                Some(ni) => {
                    if e < n_ext {
                        old_to_new[e] = external;
                        external += 1;
                    } else if pointi < ni {
                        if (v as usize) < ni {
                            old_to_new[e] = internal0;
                            internal0 += 1;
                        } else {
                            old_to_new[e] = internal1;
                            internal1 += 1;
                        }
                    } else {
                        old_to_new[e] = internal2;
                        internal2 += 1;
                    }
                }
            }
        }
    }
    let mut edges = vec![(0usize, 0usize); n_edges];
    for (e, &pair) in es.iter().enumerate() {
        edges[old_to_new[e]] = pair;
    }
    edges
}

fn build_topo(m: &PolyMesh) -> Topo<'_> {
    let nf = m.faces.len();
    let edges = calc_edges(m);
    let edge_id: HashMap<(usize, usize), usize> =
        edges.iter().enumerate().map(|(i, &e)| (e, i)).collect();
    let mut face_edges = Vec::with_capacity(nf);
    let mut edge_faces: Vec<Vec<usize>> = vec![Vec::new(); edges.len()];
    for (fi, f) in m.faces.iter().enumerate() {
        let n = f.len();
        let fe: Vec<usize> = (0..n).map(|i| edge_id[&ekey(f[i], f[(i + 1) % n])]).collect();
        for &e in &fe {
            if edge_faces[e].last() != Some(&fi) {
                edge_faces[e].push(fi);
            }
        }
        face_edges.push(fe);
    }
    let mut point_edges: Vec<Vec<usize>> = vec![Vec::new(); m.points.len()];
    for (e, &(a, b)) in edges.iter().enumerate() {
        point_edges[a].push(e);
        if b != a {
            point_edges[b].push(e);
        }
    }
    let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); m.n_cells];
    for f in 0..nf {
        cell_faces[m.owner[f]].push(f);
    }
    for f in 0..m.n_internal_faces {
        cell_faces[m.neighbour[f]].push(f);
    }
    let mut point_cells: Vec<Vec<usize>> = vec![Vec::new(); m.points.len()];
    for (c, cf) in cell_faces.iter().enumerate() {
        let mut pts: Vec<usize> = cf.iter().flat_map(|&f| m.faces[f].iter().copied()).collect();
        pts.sort_unstable();
        pts.dedup();
        for p in pts {
            point_cells[p].push(c);
        }
    }
    let mut is_boundary_edge = vec![false; edges.len()];
    for f in m.n_internal_faces..nf {
        for &e in &face_edges[f] {
            is_boundary_edge[e] = true;
        }
    }
    Topo {
        m,
        nf,
        edges,
        edge_id,
        face_edges,
        edge_faces,
        point_edges,
        cell_faces,
        point_cells,
        is_boundary_edge,
    }
}

/// `PrimitivePatch` addressing of a face list: `meshPoints` (order of first
/// appearance), `pointFaces` (ascending local faces) and the edge list in
/// `calcAddressing` order with each edge's faces.
struct PatchAddr {
    mesh_points: Vec<usize>,
    point_faces: Vec<Vec<usize>>,
    /// Edges as `(start, end)` in the direction of their lowest face.
    edges: Vec<(usize, usize)>,
    edge_faces: Vec<Vec<usize>>,
    n_internal_edges: usize,
}

fn patch_addressing(faces: &[&[usize]]) -> PatchAddr {
    let mut local_of: HashMap<usize, usize> = HashMap::new();
    let mut mesh_points: Vec<usize> = Vec::new();
    for f in faces {
        for &p in f.iter() {
            local_of.entry(p).or_insert_with(|| {
                mesh_points.push(p);
                mesh_points.len() - 1
            });
        }
    }
    let mut point_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh_points.len()];
    for (fi, f) in faces.iter().enumerate() {
        for &p in f.iter() {
            let l = local_of[&p];
            if point_faces[l].last() != Some(&fi) {
                point_faces[l].push(fi);
            }
        }
    }
    let face_edge_list = |f: &[usize]| -> Vec<(usize, usize)> {
        let n = f.len();
        (0..n).map(|i| (f[i], f[(i + 1) % n])).collect()
    };
    let fie: Vec<Vec<(usize, usize)>> = faces.iter().map(|f| face_edge_list(f)).collect();
    let same = |a: (usize, usize), b: (usize, usize)| a == b || (a.0 == b.1 && a.1 == b.0);
    let mut face_edge_no: Vec<Vec<Option<usize>>> = fie.iter().map(|e| vec![None; e.len()]).collect();
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();
    for fi in 0..faces.len() {
        let n = fie[fi].len();
        let mut nei_faces: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut nei_edge: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut n_nbrs = 0;
        for ei in 0..n {
            if face_edge_no[fi][ei].is_some() {
                continue;
            }
            let e = fie[fi][ei];
            let mut found = false;
            for &cur in &point_faces[local_of[&e.0]] {
                if cur > fi {
                    for (nei_ei, &se) in fie[cur].iter().enumerate() {
                        if same(se, e) {
                            found = true;
                            nei_faces[ei].push(cur);
                            nei_edge[ei].push(nei_ei);
                        }
                    }
                }
            }
            if found {
                n_nbrs += 1;
            }
        }
        for _ in 0..n_nbrs {
            let mut next: Option<usize> = None;
            let mut min_nei = faces.len();
            for (nf_i, nl) in nei_faces.iter().enumerate() {
                if !nl.is_empty() && nl[0] < min_nei {
                    next = Some(nf_i);
                    min_nei = nl[0];
                }
            }
            let nx = next.expect("patch addressing: internal edge insertion");
            let no = edges.len();
            edges.push(fie[fi][nx]);
            face_edge_no[fi][nx] = Some(no);
            let mut ef = vec![fi];
            for (k, &nb) in nei_faces[nx].iter().enumerate() {
                face_edge_no[nb][nei_edge[nx][k]] = Some(no);
                ef.push(nb);
            }
            edge_faces.push(ef);
            nei_faces[nx].clear();
            nei_edge[nx].clear();
        }
    }
    let n_internal_edges = edges.len();
    for fi in 0..faces.len() {
        for ei in 0..fie[fi].len() {
            if face_edge_no[fi][ei].is_none() {
                let no = edges.len();
                edges.push(fie[fi][ei]);
                face_edge_no[fi][ei] = Some(no);
                edge_faces.push(vec![fi]);
            }
        }
    }
    PatchAddr {
        mesh_points,
        point_faces,
        edges,
        edge_faces,
        n_internal_edges,
    }
}

// ───────────────────────────── feature marking ──────────────────────────────

struct Features {
    feature_faces: Vec<usize>,
    feature_edges: Vec<usize>,
    single_cell_points: Vec<usize>,
    multi_cell_points: Vec<usize>,
}

fn patch_faces_of<'a>(m: &'a PolyMesh, start: usize, size: usize) -> Vec<&'a [usize]> {
    (start..start + size).map(|f| m.faces[f].as_slice()).collect()
}

/// Port of `simpleMarkFeatures` (polyDualMesh.C), including the hash-set
/// insertion order that fixes `toc()` order.
fn simple_mark_features(
    t: &Topo<'_>,
    opts: &UpstreamOptions,
    f_centres: &[V],
    f_areas: &[V],
) -> Result<Features, MeshError> {
    let m = t.m;
    let min_cos = (opts.feature_angle_deg * std::f64::consts::PI / 180.0).r_cos();
    let mut feature_edge_set = FoamLabelHashSet::new();
    let mut single = FoamLabelHashSet::new();
    let mut multi = FoamLabelHashSet::new();

    // 1. Edges between patches: each patch's boundary edges.
    for p in &m.patches {
        let faces = patch_faces_of(m, p.start, p.size);
        let pa = patch_addressing(&faces);
        for ei in pa.n_internal_edges..pa.edges.len() {
            let (a, b) = pa.edges[ei];
            let me = t.edge_id[&ekey(a, b)];
            feature_edge_set.insert(me);
            let (e0, e1) = t.edges[me];
            single.insert(e0);
            single.insert(e1);
        }
    }

    // 2. Geometric feature edges of the whole boundary.
    let nb = t.nf - m.n_internal_faces;
    let bfaces = patch_faces_of(m, m.n_internal_faces, nb);
    let pa = patch_addressing(&bfaces);

    // checkPointManifold(false, &single): points whose boundary faces are not
    // connected through an edge.
    for (lp, pfaces) in pa.point_faces.iter().enumerate() {
        let p = pa.mesh_points[lp];
        let mut seen = vec![false; pfaces.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(i) = stack.pop() {
            for (j, &fj) in pfaces.iter().enumerate() {
                if !seen[j] && shares_edge_at_point(bfaces[pfaces[i]], bfaces[fj], p) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if seen.iter().any(|&s| !s) {
            single.insert(p);
        }
    }
    // Non-manifold boundary edges.
    for (ei, ef) in pa.edge_faces.iter().enumerate() {
        if ef.len() > 2 {
            let (a, b) = pa.edges[ei];
            single.insert(a);
            single.insert(b);
        }
    }
    let normals: Vec<V> = (0..nb)
        .map(|i| {
            let f = bfaces[i];
            let pts: Vec<V> = f.iter().map(|&p| to_v(&m.points[p])).collect();
            let a = if pts.len() == 3 {
                vscale(0.5, vcross(vsub(pts[1], pts[0]), vsub(pts[2], pts[0])))
            } else {
                face_area_only(&pts)
            };
            vnormalised(a)
        })
        .collect();
    for (ei, ef) in pa.edge_faces.iter().enumerate() {
        if ef.len() != 2 {
            continue;
        }
        let (f0, f1) = (ef[0], ef[1]);
        let (n0, n1) = (normals[f0], normals[f1]);
        if vdot(n0, n1) < min_cos {
            let (v0, v1) = pa.edges[ei];
            let me = t.edge_id[&ekey(v0, v1)];
            feature_edge_set.insert(me);
            let c1c0 = vsub(f_centres[m.n_internal_faces + f1], f_centres[m.n_internal_faces + f0]);
            if opts.concave_multi_cells && vdot(c1c0, n0) > SMALL {
                single.erase(v0);
                multi.insert(v0);
                single.erase(v1);
                multi.insert(v1);
            } else {
                if !multi.found(v0) {
                    single.insert(v0);
                }
                if !multi.found(v1) {
                    single.insert(v1);
                }
            }
        }
    }

    // 3. Feature faces: every boundary face, then face zones.
    let mut feature_face_set = if opts.esi2506_hash_reserve {
        FoamLabelHashSet::with_reserve_esi2506(nb)
    } else {
        FoamLabelHashSet::with_size(nb)
    };
    for f in m.n_internal_faces..t.nf {
        feature_face_set.insert(f);
    }
    if opts.preserve_face_zones {
        for z in &opts.face_zones {
            for &fi in &z.faces {
                if fi >= t.nf {
                    return Err(MeshError::Construction(format!(
                        "face zone {}: face {fi} out of range",
                        z.name
                    )));
                }
                feature_face_set.insert(fi);
                for &p in &m.faces[fi] {
                    single.erase(p);
                    multi.insert(p);
                }
                for &e in &t.face_edges[fi] {
                    feature_edge_set.insert(e);
                }
            }
        }
    }
    let _ = f_areas;
    let mut r = Features {
        feature_faces: feature_face_set.toc(),
        feature_edges: feature_edge_set.toc(),
        single_cell_points: single.toc(),
        multi_cell_points: multi.toc(),
    };
    if opts.split_all_faces {
        r.feature_edges = (0..t.edges.len()).collect();
        r.feature_faces = (0..t.nf).collect();
    }
    Ok(r)
}

fn shares_edge_at_point(a: &[usize], b: &[usize], p: usize) -> bool {
    let nbrs = |f: &[usize]| -> Vec<usize> {
        let n = f.len();
        match f.iter().position(|&q| q == p) {
            Some(i) => vec![f[(i + n - 1) % n], f[(i + 1) % n]],
            None => Vec::new(),
        }
    };
    let (na, nb) = (nbrs(a), nbrs(b));
    na.iter().any(|x| nb.contains(x))
}

/// `face::area` for a polygon with more than three points (faceTemplates.C).
fn face_area_only(ps: &[V]) -> V {
    let n = ps.len();
    let mut p_avg = ps[0];
    for p in &ps[1..] {
        p_avg = vadd(p_avg, *p);
    }
    let p_avg = vdiv(p_avg, n as f64);
    let mut sum_a = [0.0; 3];
    for i in 0..n {
        let (p, pn) = (ps[i], ps[(i + 1) % n]);
        sum_a = vadd(sum_a, vcross(vsub(pn, p), vsub(p_avg, p)));
    }
    vscale(1.0 / 2.0, sum_a)
}

// ───────────────────────── edgeFaceCirculator ───────────────────────────────

/// Port of `Foam::edgeFaceCirculator` (src/meshTools/edgeFaceCirculator).
#[derive(Clone)]
struct Circ {
    face: Option<usize>,
    owner_side: bool,
    index: Option<usize>,
    is_boundary_edge: bool,
    start_face: Option<usize>,
}

fn get_min_index(f: &[usize], v0: usize, v1: usize) -> Option<usize> {
    let n = f.len();
    let mut fp = f.iter().position(|&x| x == v0)?;
    let fp_min1 = (fp + n - 1) % n;
    if f[fp_min1] == v1 {
        fp = fp_min1;
    } else {
        let fp_plus1 = (fp + 1) % n;
        if f[fp_plus1] != v1 {
            return None;
        }
    }
    Some(fp)
}

impl Circ {
    fn new(face: usize, owner_side: bool, index: usize, is_boundary_edge: bool) -> Self {
        Self {
            face: Some(face),
            owner_side,
            index: Some(index),
            is_boundary_edge,
            start_face: Some(face),
        }
    }
    fn ended(&self) -> bool {
        self.face.is_none() && self.index.is_none()
    }
    fn face_label(&self) -> usize {
        self.face.expect("circulator at end")
    }
    fn set_end(&mut self) {
        self.face = None;
        self.index = None;
    }
    fn other_face(&mut self, t: &Topo<'_>, celli: usize) -> Result<(), MeshError> {
        let cur = self.face_label();
        let f = &t.m.faces[cur];
        let idx = self.index.expect("index");
        let (v0, v1) = (f[idx], f[(idx + 1) % f.len()]);
        for &face_b in &t.cell_faces[celli] {
            if face_b != cur {
                if let Some(fp) = get_min_index(&t.m.faces[face_b], v0, v1) {
                    self.index = Some(fp);
                    self.face = Some(face_b);
                    return Ok(());
                }
            }
        }
        Err(MeshError::Construction(format!(
            "edgeFaceCirculator: no next face stepping through cell {celli} along edge ({v0},{v1})"
        )))
    }
    fn cell_label(&self, t: &Topo<'_>) -> Option<usize> {
        let f = self.face_label();
        if self.owner_side {
            Some(t.m.owner[f])
        } else if t.m.is_internal_face(f) {
            Some(t.m.neighbour[f])
        } else {
            None
        }
    }
    /// `operator++`.
    fn step(&mut self, t: &Topo<'_>) -> Result<(), MeshError> {
        let f = self
            .face
            .ok_or_else(|| MeshError::Construction("edgeFaceCirculator: already at end".into()))?;
        if self.owner_side {
            let celli = t.m.owner[f];
            self.other_face(t, celli)?;
            self.owner_side = t.m.owner[self.face_label()] != celli;
            if !self.is_boundary_edge && self.face == self.start_face {
                self.set_end();
            }
        } else if t.m.is_internal_face(f) {
            let celli = t.m.neighbour[f];
            self.other_face(t, celli)?;
            self.owner_side = t.m.owner[self.face_label()] != celli;
            if !self.is_boundary_edge && self.face == self.start_face {
                self.set_end();
            }
        } else {
            self.set_end();
        }
        Ok(())
    }
    fn same_order(&self, t: &Topo<'_>, v0: usize) -> bool {
        let f = &t.m.faces[self.face_label()];
        self.owner_side != (f[self.index.expect("index")] == v0)
    }
    fn set_canonical(&mut self, t: &Topo<'_>) -> Result<(), MeshError> {
        if self.is_boundary_edge {
            let mut i = 0;
            loop {
                let f = self.face_label();
                if t.m.is_internal_face(f) {
                    let celli = if self.owner_side { t.m.neighbour[f] } else { t.m.owner[f] };
                    self.other_face(t, celli)?;
                    self.owner_side = t.m.owner[self.face_label()] == celli;
                } else if self.owner_side {
                    break;
                } else {
                    let celli = t.m.owner[f];
                    self.other_face(t, celli)?;
                    self.owner_side = t.m.owner[self.face_label()] == celli;
                }
                i += 1;
                if i >= 1000 {
                    return Err(MeshError::Construction(
                        "edgeFaceCirculator: walked 1000 cells without reaching a boundary face".into(),
                    ));
                }
            }
            self.owner_side = true;
            self.start_face = self.face;
        } else {
            let mut min_face = self.face_label();
            let mut min_owner_side = self.owner_side;
            let mut min_index = self.index;
            loop {
                self.step(t)?;
                if self.ended() {
                    break;
                }
                let f = self.face_label();
                if !t.m.is_internal_face(f) {
                    return Err(MeshError::Construction(format!(
                        "edgeFaceCirculator: reached boundary face {f} walking an internal edge"
                    )));
                }
                if f < min_face {
                    min_face = f;
                    min_owner_side = self.owner_side;
                    min_index = self.index;
                }
            }
            self.face = Some(min_face);
            self.owner_side = min_owner_side;
            self.index = min_index;
            self.start_face = self.face;
        }
        Ok(())
    }
}

// ───────────────────────────── meshDualiser ─────────────────────────────────

struct TopoFace {
    verts: Vec<usize>,
    own: usize,
    nei: Option<usize>,
    patch: Option<usize>,
}

struct Dualiser<'a, 'b> {
    t: &'b Topo<'a>,
    point_to_dual_cells: Vec<Vec<usize>>,
    point_to_dual_point: Vec<Option<usize>>,
    cell_to_dual_point: Vec<usize>,
    face_to_dual_point: Vec<Option<usize>>,
    edge_to_dual_point: Vec<Option<usize>>,
    points: Vec<V>,
    n_dual_cells: usize,
    faces: Vec<TopoFace>,
}

impl<'a, 'b> Dualiser<'a, 'b> {
    fn find_dual_cell(&self, celli: usize, pointi: usize) -> usize {
        let dc = &self.point_to_dual_cells[pointi];
        if dc.len() == 1 {
            dc[0]
        } else {
            let index = self.t.point_cells[pointi]
                .iter()
                .position(|&c| c == celli)
                .expect("cell uses point");
            dc[index]
        }
    }

    fn add_point(&mut self, p: V) -> usize {
        self.points.push(p);
        self.points.len() - 1
    }

    fn add_cell(&mut self) -> usize {
        self.n_dual_cells += 1;
        self.n_dual_cells - 1
    }

    fn generate_dual_boundary_edges(&mut self, pointi: usize) {
        let t = self.t;
        for &e in &t.point_edges[pointi] {
            if self.edge_to_dual_point[e].is_none() && t.is_boundary_edge[e] {
                let (a, b) = t.edges[e];
                let c = vscale(0.5, vadd(to_v(&t.m.points[a]), to_v(&t.m.points[b])));
                self.edge_to_dual_point[e] = Some(self.add_point(c));
            }
        }
    }

    fn same_dual_cell(&self, facei: usize, pointi: usize) -> bool {
        let m = self.t.m;
        self.find_dual_cell(m.owner[facei], pointi) == self.find_dual_cell(m.neighbour[facei], pointi)
    }

    fn add_internal_face(
        &mut self,
        edge_order: bool,
        dual_cell0: usize,
        dual_cell1: usize,
        verts: &[usize],
    ) -> Result<(), MeshError> {
        if dual_cell0 == dual_cell1 {
            return Err(MeshError::Construction(format!(
                "meshDualiser: internal dual face between a dual cell and itself ({dual_cell0})"
            )));
        }
        let mut v = verts.to_vec();
        if edge_order != (dual_cell0 < dual_cell1) {
            v.reverse();
        }
        let (own, nei) = if dual_cell0 < dual_cell1 {
            (dual_cell0, dual_cell1)
        } else {
            (dual_cell1, dual_cell0)
        };
        self.faces.push(TopoFace { verts: v, own, nei: Some(nei), patch: None });
        Ok(())
    }

    fn add_boundary_face(&mut self, dual_celli: usize, patchi: usize, verts: &[usize]) {
        self.faces.push(TopoFace {
            verts: verts.to_vec(),
            own: dual_celli,
            nei: None,
            patch: Some(patchi),
        });
    }

    #[allow(clippy::too_many_lines)]
    fn create_faces_around_edge(
        &mut self,
        split_face: bool,
        edge_i: usize,
        start_facei: usize,
        done_efaces: &mut [bool],
    ) -> Result<(), MeshError> {
        let t = self.t;
        let (e0, e1) = t.edges[edge_i];
        let efaces = &t.edge_faces[edge_i];
        let is_bnd = t.is_boundary_edge[edge_i];
        let fp = get_min_index(&t.m.faces[start_facei], e0, e1)
            .ok_or_else(|| MeshError::Construction("createFacesAroundEdge: edge not on face".into()))?;
        let mut ie = Circ::new(start_facei, true, fp, is_bnd);
        ie.set_canonical(t)?;
        let edge_order = ie.same_order(t, e0);
        let start_face_label = ie.face_label();
        let fidx = |f: usize| efaces.iter().position(|&x| x == f).expect("edge face");

        let mut verts: Vec<usize> = Vec::new();
        if let Some(d) = self.edge_to_dual_point[edge_i] {
            verts.push(d);
        }
        if let Some(d) = self.face_to_dual_point[ie.face_label()] {
            done_efaces[fidx(ie.face_label())] = true;
            verts.push(d);
        }
        let c0 = ie.cell_label(t).expect("canonical start has a cell");
        verts.push(self.cell_to_dual_point[c0]);
        let mut current0 = self.find_dual_cell(c0, e0);
        let mut current1 = self.find_dual_cell(c0, e1);

        ie.step(t)?;
        loop {
            let facei = ie.face_label();
            done_efaces[fidx(facei)] = true;
            if let Some(d) = self.face_to_dual_point[facei] {
                verts.push(d);
            }
            let Some(celli) = ie.cell_label(t) else {
                break;
            };
            let dual0 = self.find_dual_cell(celli, e0);
            let dual1 = self.find_dual_cell(celli, e1);
            if split_face || dual0 != current0 || dual1 != current1 {
                self.add_internal_face(edge_order, current0, current1, &verts)?;
                current0 = dual0;
                current1 = dual1;
                verts.clear();
                if let Some(d) = self.edge_to_dual_point[edge_i] {
                    verts.push(d);
                }
                if let Some(d) = self.face_to_dual_point[facei] {
                    verts.push(d);
                }
            }
            verts.push(self.cell_to_dual_point[celli]);
            ie.step(t)?;
            if ie.ended() {
                if !is_bnd {
                    if let Some(sd) = self.face_to_dual_point[start_face_label] {
                        if !verts.contains(&sd) {
                            verts.push(sd);
                        }
                    }
                }
                break;
            }
        }
        self.add_internal_face(edge_order, current0, current1, &verts)
    }

    fn create_face_from_internal_face(&mut self, facei: usize, fp: &mut usize) -> Result<(), MeshError> {
        let t = self.t;
        let f = &t.m.faces[facei];
        let n = f.len();
        let fedges = &t.face_edges[facei];
        let own = t.m.owner[facei];
        let nei = t.m.neighbour[facei];
        let mut verts: Vec<usize> = vec![
            self.face_to_dual_point[facei].expect("feature face point"),
            self.edge_to_dual_point[fedges[*fp]].expect("starting feature edge point"),
        ];
        *fp = (*fp + 1) % n;
        let current0 = self.find_dual_cell(own, f[*fp]);
        let current1 = self.find_dual_cell(nei, f[*fp]);
        for _ in 0..n {
            if let Some(d) = self.point_to_dual_point[f[*fp]] {
                verts.push(d);
            }
            let edge_i = fedges[*fp];
            if let Some(d) = self.edge_to_dual_point[edge_i] {
                verts.push(d);
            }
            let next_fp = (*fp + 1) % n;
            let dual0 = self.find_dual_cell(own, f[next_fp]);
            let dual1 = self.find_dual_cell(nei, f[next_fp]);
            if dual0 != current0 || dual1 != current1 {
                if self.edge_to_dual_point[edge_i].is_none() {
                    return Err(MeshError::Construction(format!(
                        "meshDualiser: face {facei}: no feature edge between {} and {} although they have different dual cells",
                        f[*fp], f[next_fp]
                    )));
                }
                return self.add_internal_face(true, current0, current1, &verts);
            }
            *fp = next_fp;
        }
        Ok(())
    }

    fn step_to_next_boundary_face(&self, facei: usize, prev_fp: usize) -> Result<usize, MeshError> {
        let t = self.t;
        let mut circ = Circ::new(facei, true, prev_fp, true);
        loop {
            circ.step(t)?;
            if !t.m.is_internal_face(circ.face_label()) {
                return Ok(circ.face_label());
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn create_faces_around_boundary_point(
        &mut self,
        patchi: usize,
        patch_start: usize,
        patch_size: usize,
        point_i: usize,
        pfaces: &[usize],
        start_facei: usize,
        done_pfaces: &mut [bool],
    ) -> Result<(), MeshError> {
        let t = self.t;
        let m = t.m;
        let in_patch = |f: usize| f >= patch_start && f < patch_start + patch_size;
        let pidx = |f: usize| pfaces.iter().position(|&x| x == f - patch_start).expect("patch face of point");
        if self.point_to_dual_point[point_i].is_none() {
            let mut facei = start_facei;
            let mut verts: Vec<usize> = Vec::new();
            loop {
                let index = pidx(facei);
                if done_pfaces[index] {
                    break;
                }
                done_pfaces[index] = true;
                verts.push(self.face_to_dual_point[facei].expect("boundary face point"));
                let dual_celli = self.find_dual_cell(m.owner[facei], point_i);
                let f = &m.faces[facei];
                let fp = f.iter().position(|&x| x == point_i).expect("point on face");
                let prev_fp = (fp + f.len() - 1) % f.len();
                let edge_i = t.face_edges[facei][prev_fp];
                if let Some(d) = self.edge_to_dual_point[edge_i] {
                    verts.push(d);
                }
                facei = self.step_to_next_boundary_face(facei, prev_fp)?;
                if !in_patch(facei) {
                    return Err(MeshError::Construction(format!(
                        "meshDualiser: walked from patch {patchi} to face {facei} on another patch"
                    )));
                }
                if dual_celli != self.find_dual_cell(m.owner[facei], point_i) {
                    return Err(MeshError::Construction(format!(
                        "meshDualiser: different dual cells but no feature edge in between at point {point_i}"
                    )));
                }
            }
            let dual_celli = self.find_dual_cell(m.owner[facei], point_i);
            self.add_boundary_face(dual_celli, patchi, &verts);
        } else {
            let mut facei = start_facei;
            let mut verts: Vec<usize> = vec![self.point_to_dual_point[point_i].expect("feature point")];
            {
                let fedges = &t.face_edges[facei];
                let f = &m.faces[facei];
                let fp = f.iter().position(|&x| x == point_i).expect("point on face");
                if let Some(d) = self.edge_to_dual_point[fedges[fp]] {
                    verts.push(d);
                }
            }
            loop {
                let index = pidx(facei);
                if done_pfaces[index] {
                    break;
                }
                done_pfaces[index] = true;
                verts.push(self.face_to_dual_point[facei].expect("boundary face point"));
                let f = &m.faces[facei];
                let prev_fp = (f.iter().position(|&x| x == point_i).expect("point on face") + f.len() - 1) % f.len();
                let edge_i = t.face_edges[facei][prev_fp];
                if let Some(d) = self.edge_to_dual_point[edge_i] {
                    verts.push(d);
                    let dc = self.find_dual_cell(m.owner[facei], point_i);
                    self.add_boundary_face(dc, patchi, &verts);
                    verts.clear();
                    verts.push(self.point_to_dual_point[point_i].expect("feature point"));
                    verts.push(d);
                }
                facei = self.step_to_next_boundary_face(facei, prev_fp)?;
                if !(facei != start_facei && in_patch(facei)) {
                    break;
                }
            }
            if verts.len() > 2 {
                let dc = self.find_dual_cell(m.owner[facei], point_i);
                self.add_boundary_face(dc, patchi, &verts);
            }
        }
        Ok(())
    }
}

// ───────────────────────────── driver ───────────────────────────────────────

/// The polyhedral dual of `primal` by OpenFOAM's `polyDualMesh` utility
/// (`simpleMarkFeatures` + `meshDualiser` + `polyTopoChange` ordering).
///
/// Faces come out in `polyTopoChange::changeMesh` order (internal faces
/// upper-triangular in the dual cells, then patch by patch), and each dual
/// patch is the primal patch of the same index, so the result writes
/// directly.
///
/// # Errors
/// [`MeshError::Construction`] when the primal mesh breaks an assumption the
/// utility also aborts on (non-manifold fans, a feature-less change of dual
/// cell across a face, ...).
pub fn poly_dual_mesh_upstream(primal: &PolyMesh, opts: &UpstreamOptions) -> Result<DualMesh, MeshError> {
    let t = build_topo(primal);
    let m = primal;
    let nf = t.nf;

    // Geometry the dual points come from.
    let mut f_centres: Vec<V> = Vec::with_capacity(nf);
    let mut f_areas: Vec<V> = Vec::with_capacity(nf);
    for f in &m.faces {
        let pts: Vec<V> = f.iter().map(|&p| to_v(&m.points[p])).collect();
        let (a, c) = area_and_centre_v(opts.centres, &pts);
        f_areas.push(a);
        f_centres.push(c);
    }
    let fc_v: Vec<Vector3> = f_centres.iter().map(|&c| from_v(c)).collect();
    let fa_v: Vec<Vector3> = f_areas.iter().map(|&c| from_v(c)).collect();
    let (cell_centres, _) = cell_centres_and_volumes(m.n_cells, &m.owner, &m.neighbour, &fc_v, &fa_v);

    let feat = simple_mark_features(&t, opts, &f_centres, &f_areas)?;
    let split_face = opts.split_all_faces;

    // Checks of setRefinement.
    if split_face {
        let mut is_f = vec![false; nf];
        for &f in &feat.feature_faces {
            is_f[f] = true;
        }
        if let Some(f) = is_f.iter().position(|&x| !x) {
            return Err(MeshError::Construction(format!(
                "meshDualiser: split-face mode but face {f} is not a feature face"
            )));
        }
        let mut is_e = vec![false; t.edges.len()];
        for &e in &feat.feature_edges {
            is_e[e] = true;
        }
        if let Some(e) = is_e.iter().position(|&x| !x) {
            return Err(MeshError::Construction(format!(
                "meshDualiser: split-face mode but edge {e} is not a feature edge"
            )));
        }
    } else {
        let mut is_f = vec![false; nf];
        for &f in &feat.feature_faces {
            is_f[f] = true;
        }
        if let Some(f) = (m.n_internal_faces..nf).find(|&f| !is_f[f]) {
            return Err(MeshError::Construction(format!(
                "meshDualiser: boundary face {f} is not a feature face"
            )));
        }
    }

    let mut d = Dualiser {
        t: &t,
        point_to_dual_cells: vec![Vec::new(); m.points.len()],
        point_to_dual_point: vec![None; m.points.len()],
        cell_to_dual_point: vec![0; m.n_cells],
        face_to_dual_point: vec![None; nf],
        edge_to_dual_point: vec![None; t.edges.len()],
        points: Vec::new(),
        n_dual_cells: 0,
        faces: Vec::new(),
    };

    // Dual cells (from points).
    for &p in &feat.single_cell_points {
        let dp = d.add_point(to_v(&m.points[p]));
        d.point_to_dual_point[p] = Some(dp);
        let c = d.add_cell();
        d.point_to_dual_cells[p] = vec![c];
    }
    for &p in &feat.multi_cell_points {
        if !d.point_to_dual_cells[p].is_empty() {
            return Err(MeshError::Construction(format!(
                "meshDualiser: point {p} is in both the single-cell and the multi-cell feature sets"
            )));
        }
        let dp = d.add_point(to_v(&m.points[p]));
        d.point_to_dual_point[p] = Some(dp);
        let n = t.point_cells[p].len();
        let cells: Vec<usize> = (0..n).map(|_| d.add_cell()).collect();
        d.point_to_dual_cells[p] = cells;
    }
    for p in 0..m.points.len() {
        if d.point_to_dual_cells[p].is_empty() {
            let c = d.add_cell();
            d.point_to_dual_cells[p] = vec![c];
        }
    }

    // Dual points (cell centres, feature faces, feature edges).
    for c in 0..m.n_cells {
        d.cell_to_dual_point[c] = d.add_point(to_v(&cell_centres[c]));
    }
    for &f in &feat.feature_faces {
        d.face_to_dual_point[f] = Some(d.add_point(f_centres[f]));
    }
    for f in 0..m.n_internal_faces {
        if d.face_to_dual_point[f].is_none() {
            for &p in &m.faces[f] {
                if d.find_dual_cell(m.owner[f], p) != d.find_dual_cell(m.neighbour[f], p) {
                    d.face_to_dual_point[f] = Some(d.add_point(f_centres[f]));
                    break;
                }
            }
        }
    }
    let edge_centre = |e: usize| -> V {
        let (a, b) = t.edges[e];
        vscale(0.5, vadd(to_v(&m.points[a]), to_v(&m.points[b])))
    };
    for &e in &feat.feature_edges {
        d.edge_to_dual_point[e] = Some(d.add_point(edge_centre(e)));
    }
    let edge_cells: Vec<Vec<usize>> = t
        .edge_faces
        .iter()
        .map(|fs| {
            let mut cs: Vec<usize> = Vec::new();
            for &f in fs {
                cs.push(m.owner[f]);
                if m.is_internal_face(f) {
                    cs.push(m.neighbour[f]);
                }
            }
            cs.sort_unstable();
            cs.dedup();
            cs
        })
        .collect();
    for e in 0..t.edges.len() {
        if d.edge_to_dual_point[e].is_none() {
            let (a, b) = t.edges[e];
            let ec = &edge_cells[e];
            let dual_e0 = d.find_dual_cell(ec[0], a);
            let dual_e1 = d.find_dual_cell(ec[0], b);
            for &c in &ec[1..] {
                if dual_e0 != d.find_dual_cell(c, a) || dual_e1 != d.find_dual_cell(c, b) {
                    d.edge_to_dual_point[e] = Some(d.add_point(edge_centre(e)));
                    break;
                }
            }
        }
    }
    for &p in &feat.single_cell_points {
        d.generate_dual_boundary_edges(p);
    }
    for &p in &feat.multi_cell_points {
        d.generate_dual_boundary_edges(p);
    }

    // Faces from edges.
    for e in 0..t.edges.len() {
        let efaces = &t.edge_faces[e];
        let mut done = vec![false; efaces.len()];
        for i in 0..efaces.len() {
            if !done[i] {
                d.create_faces_around_edge(split_face, e, efaces[i], &mut done)?;
            }
        }
    }

    // Faces from feature faces.
    for facei in 0..nf {
        if d.face_to_dual_point[facei].is_some() && m.is_internal_face(facei) {
            let f = &m.faces[facei];
            let n = f.len();
            let mut fp = 0usize;
            loop {
                let mut found_start = false;
                loop {
                    if d.edge_to_dual_point[t.face_edges[facei][fp]].is_some()
                        && !d.same_dual_cell(facei, f[(fp + 1) % n])
                    {
                        found_start = true;
                        break;
                    }
                    fp = (fp + 1) % n;
                    if fp == 0 {
                        break;
                    }
                }
                if !found_start {
                    break;
                }
                d.create_face_from_internal_face(facei, &mut fp)?;
                if fp == 0 {
                    break;
                }
            }
        }
    }

    // Boundary faces.
    for (patchi, p) in m.patches.iter().enumerate() {
        let faces = patch_faces_of(m, p.start, p.size);
        let pa = patch_addressing(&faces);
        for (lp, pfaces) in pa.point_faces.iter().enumerate() {
            let point_i = pa.mesh_points[lp];
            let mut done = vec![false; pfaces.len()];
            for i in 0..pfaces.len() {
                if !done[i] {
                    d.create_faces_around_boundary_point(
                        patchi,
                        p.start,
                        p.size,
                        point_i,
                        pfaces,
                        p.start + pfaces[i],
                        &mut done,
                    )?;
                }
            }
        }
    }

    // polyTopoChange::compact + getFaceOrder (orderCells = false).
    let n_dual_cells = d.n_dual_cells;
    let tfaces = d.faces;
    let n_active = tfaces.len();
    let mut cell_faces_pt: Vec<Vec<usize>> = vec![Vec::new(); n_dual_cells];
    for (i, f) in tfaces.iter().enumerate() {
        cell_faces_pt[f.own].push(i);
    }
    for (i, f) in tfaces.iter().enumerate() {
        if let Some(n) = f.nei {
            cell_faces_pt[n].push(i);
        }
    }
    let mut old_to_new = vec![usize::MAX; n_active];
    let mut new_facei = 0usize;
    for (celli, cf) in cell_faces_pt.iter().enumerate() {
        let mut nbr: Vec<i64> = Vec::with_capacity(cf.len());
        for &fi in cf {
            let f = &tfaces[fi];
            match f.nei {
                Some(mut n) => {
                    if n == celli {
                        n = f.own;
                    }
                    nbr.push(if celli < n { n as i64 } else { -1 });
                }
                None => nbr.push(-1),
            }
        }
        let mut order: Vec<usize> = (0..cf.len()).collect();
        order.sort_by_key(|&i| nbr[i]); // stable, like sortedOrder
        for i in order {
            if nbr[i] != -1 {
                old_to_new[cf[i]] = new_facei;
                new_facei += 1;
            }
        }
    }
    let n_internal = new_facei;
    let n_patches = m.patches.len().max(1);
    let mut patch_sizes = vec![0usize; n_patches];
    for f in &tfaces {
        if let Some(p) = f.patch {
            patch_sizes[p] += 1;
        }
    }
    let mut work_starts = vec![0usize; n_patches];
    let mut s = n_internal;
    for p in 0..n_patches {
        work_starts[p] = s;
        s += patch_sizes[p];
    }
    for (i, f) in tfaces.iter().enumerate() {
        if let Some(p) = f.patch {
            old_to_new[i] = work_starts[p];
            work_starts[p] += 1;
        }
    }
    if let Some(i) = old_to_new.iter().position(|&x| x == usize::MAX) {
        return Err(MeshError::Construction(format!(
            "polyTopoChange: no new position for dual face {i} (a boundary face without a patch)"
        )));
    }
    let mut slots: Vec<Option<TopoFace>> = (0..n_active).map(|_| None).collect();
    for (i, f) in tfaces.into_iter().enumerate() {
        slots[old_to_new[i]] = Some(f);
    }
    let mut faces = Vec::with_capacity(n_active);
    let mut owner = Vec::with_capacity(n_active);
    let mut neighbour = Vec::with_capacity(n_active);
    let mut face_patch = Vec::with_capacity(n_active);
    for s in slots {
        let f = s.expect("every slot filled");
        faces.push(f.verts);
        owner.push(f.own);
        neighbour.push(f.nei);
        face_patch.push(f.patch);
    }
    let mut cell_is_interior = vec![true; n_dual_cells];
    for (i, &o) in owner.iter().enumerate() {
        if neighbour[i].is_none() {
            cell_is_interior[o] = false;
        }
    }
    Ok(DualMesh {
        points: d.points.iter().map(|&p| from_v(p)).collect(),
        faces,
        owner,
        neighbour,
        face_patch,
        n_cells: n_dual_cells,
        cell_is_interior,
    })
}

/// The feature sets `simpleMarkFeatures` hands to `meshDualiser`, each in
/// `labelHashSet::toc()` order (the order that numbers the dual points and
/// cells). The utility dumps the same sets to `featureFaces.obj`,
/// `featureEdges.obj`, `singleCellFeaturePoints.obj` and
/// `multiCellFeaturePoints.obj`, so this is what the differential compares
/// first (gh:#1472).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureSets {
    /// Face labels.
    pub faces: Vec<usize>,
    /// Edge labels, in [`primal_edges`] numbering.
    pub edges: Vec<usize>,
    /// Points that become one dual cell.
    pub single_cell_points: Vec<usize>,
    /// Points that become several dual cells.
    pub multi_cell_points: Vec<usize>,
}

/// `simpleMarkFeatures` on `primal` (no dual is built).
///
/// # Errors
/// [`MeshError::Construction`] for a face-zone label out of range.
pub fn upstream_feature_sets(primal: &PolyMesh, opts: &UpstreamOptions) -> Result<FeatureSets, MeshError> {
    let t = build_topo(primal);
    let mut f_centres: Vec<V> = Vec::with_capacity(t.nf);
    let mut f_areas: Vec<V> = Vec::with_capacity(t.nf);
    for f in &primal.faces {
        let pts: Vec<V> = f.iter().map(|&p| to_v(&primal.points[p])).collect();
        let (a, c) = area_and_centre_v(opts.centres, &pts);
        f_areas.push(a);
        f_centres.push(c);
    }
    let r = simple_mark_features(&t, opts, &f_centres, &f_areas)?;
    Ok(FeatureSets {
        faces: r.feature_faces,
        edges: r.feature_edges,
        single_cell_points: r.single_cell_points,
        multi_cell_points: r.multi_cell_points,
    })
}

/// The mesh edges in OpenFOAM's `primitiveMesh::calcEdges` numbering for an
/// unordered polyMesh: `(min, max)` point pairs in lexicographic order.
pub fn primal_edges(primal: &PolyMesh) -> Vec<(usize, usize)> {
    build_topo(primal).edges
}
