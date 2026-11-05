// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/
//     boundaryLayerOptimisation/{boundaryLayerOptimisation.C,
//       boundaryLayerOptimisationFunctions.C (calculateHairEdges,
//         optimiseLayersAtExittingFaces, optimiseLayer),
//       boundaryLayerOptimisationNormals.C (calculateNormalVectors,
//         calculateHairVectorsAtTheBoundary,
//         optimiseHairNormalsAtTheBoundary, optimiseHairNormalsInside),
//       boundaryLayerOptimisationThickness.C (hairEdgesAtBndFace,
//         calculateThickness, calculateThicknessOverCell,
//         optimiseThicknessVariation)}
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/optimizeMeshFV.C
//     (meshOptimizer::optimizeBoundaryLayer, untangleBoundaryLayer)
//   meshLibrary/tetMesh/tetMeshGenerator/tetMeshGenerator.C
//     (optimiseFinalMesh, refBoundaryLayers: the order of the calls)
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (nearestPointOnTheEdge, lineFaceIntersection, triLineIntersection)
// Source: github.com/DAFoam/cfmesh commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae (2026-02-17), the cfMesh v1.1
// line (boundaryLayerOptimisation is not in wyldckat/cfMesh 6cf1d211f9cf,
// v1.0, which the rest of this crate cites).
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later, conveyed here
// under GPL-3.0-only.
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

//! cfMesh's **boundary-layer optimisation** (gh:#1215): smooth the layer's
//! "hair" directions and reduce its thickness locally until neighbouring
//! hairs and the layer cells agree, then let the volume optimiser repair the
//! cells around it. [`add_boundary_layers_cfmesh`] is
//! [`LayerQualityStrategy::CfMeshOptimise`].
//!
//! A *hair edge* is the edge of a layer cell from a wall point to the point
//! the layer's inner face meets: in this crate's layers, the column of a wall
//! point from the wall to its top. cfMesh optimises the layer while it is one
//! cell thick and splits it into the requested layers afterwards
//! (`tetMeshGenerator::generateMesh`: `optimiseFinalMesh`, then
//! `refBoundaryLayers`), and so does this port.
//!
//! The driver runs, in upstream's order:
//!
//! 1. one layer at the per-point cap (the same columns as
//!    [`LayerQualityStrategy::BackOffOnly`]'s first try, no global back-off);
//! 2. `optimiseFinalMesh`: `optimizeMeshFV` (its Laplace pass is not ported;
//!    its `untangleMeshFV(10, 50, 2)` is), `optimizeLowQualityFaces(10)`,
//!    `optimizeBoundaryLayer` (`optimiseLayer`, then the layer cells' points
//!    locked and `untangleMeshFV(1, 50, 0)` and `untangleMeshFV(2, 50, 0)` on
//!    the rest), `untangleMeshFV(10, 50, 2)`;
//! 3. `refBoundaryLayers`: each hair split by the requested grading, the
//!    split points on the straight hair (cfMesh's `refineBoundaryLayers`
//!    splits along the hair edge);
//! 4. `untangleBoundaryLayer`: `optimizeLowQualityFaces` with the layer's
//!    points locked, then `untangleMeshFV(2, 50, 1, relaxed)` with none.
//!
//! **Not ported, said so:** `meshSurfaceOptimizer::optimizeSurface(2)` inside
//! `optimiseLayer` (it moves the wall points on the surface; here the wall
//! points never move), the Laplace smoothing of `optimizeMeshFV`, everything
//! in [`crate::mesh_optimizer`]'s list, and the parallel exchanges. The hair
//! classification is this crate's border classification ([`LayerVertex`]):
//! a hair at a point touching only layer faces is `INSIDE`; one at a point
//! touching one layer-free patch is `BOUNDARY | ATEDGE` (it lies in that
//! patch's plane); one touching two is `BOUNDARY | ATCORNER | FEATUREEDGE`
//! (it runs along their shared edge). cfMesh derives the same classes from
//! `detectBoundaryLayers` and `meshSurfacePartitioner`.
//!
//! The settings are those `meshOptimizer::optimizeBoundaryLayer` gets
//! (`boundaryLayerOptimisation(mesh, mse)`): `maxNumIterations 5`,
//! `nSmoothNormals 5`, `relThicknessTol 0.15`, `featureSizeFactor 0.3`,
//! `reCalculateNormals true` ([`BoundaryLayerOptimisationControls`]).
//! cfMesh has no `featureAngle` setting in this class.

use crate::layers::{assemble_layered, layer_columns, layer_setup, LayerOutcome, LayerQualityStrategy, LayerSetup, LayerVertex};
use crate::math::Vec3;
use crate::mesh_optimizer::{bad_faces_at_the_boundary, find_bad_faces, find_low_quality_faces, optimize_low_quality_faces, untangle_mesh_fv, untangle_mesh_fv_bounded};
use crate::volume_mesh::{cells_faces, from_cell_faces, VolumeMesh};
use petir::mathf::RealMath;
use std::collections::{BTreeMap, BTreeSet};

/// `boundaryLayerOptimisation`'s settings (`optimisationParameters` in
/// `meshDict`). Defaults: the values of the constructor
/// `meshOptimizer::optimizeBoundaryLayer` uses.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryLayerOptimisationControls {
    /// `maxNumIterations`: passes of `optimiseLayer`'s loop.
    pub max_num_iterations: usize,
    /// End `untangleMeshFV`'s global loop when a global iteration removes no
    /// bad face (gh:#1241, [`crate::mesh_optimizer::untangle_mesh_fv_bounded`]):
    /// upstream's boundary moves between global iterations are not ported,
    /// so the later ones repeat futile interior passes. `true` by default;
    /// `false` runs all ten, as before.
    pub stop_untangling_on_stall: bool,
    /// `nSmoothNormals`: smoothing passes over the hair directions.
    pub n_smooth_normals: usize,
    /// `relThicknessTol`: the largest change of layer height between
    /// neighbouring hairs, per unit distance between them.
    pub rel_thickness_tol: f64,
    /// `featureSizeFactor`: the share of the local feature size a hair may
    /// take.
    pub feature_size_factor: f64,
    /// `reCalculateNormals`: start from the surface normals (true) or from
    /// the hairs as they are.
    pub recalculate_normals: bool,
}

impl Default for BoundaryLayerOptimisationControls {
    fn default() -> Self {
        BoundaryLayerOptimisationControls {
            max_num_iterations: 5,
            stop_untangling_on_stall: true,
            n_smooth_normals: 5,
            rel_thickness_tol: 0.15,
            feature_size_factor: 0.3,
            recalculate_normals: true,
        }
    }
}

/// `boundaryLayerOptimisation::hairEdgeTypes`.
const BOUNDARY: u8 = 1;
const INSIDE: u8 = 2;
const FEATUREEDGE: u8 = 4;
const ATEDGE: u8 = 8;
const ATCORNER: u8 = 16;

/// One hair edge: wall point `start` to layer top `end`, mesh indices.
#[derive(Debug, Clone, Copy)]
struct Hair {
    start: usize,
    end: usize,
    kind: u8,
}

/// The layer's hairs and their neighbourhoods on the one-layer mesh.
struct Hairs {
    hairs: Vec<Hair>,
    /// Hair of each wall point (by the point's index in the one-layer mesh).
    at_point: BTreeMap<usize, usize>,
    /// `hairEdgesNearHairEdge`: hairs across a side face of a layer cell.
    near: Vec<Vec<usize>>,
    /// Boundary faces around each hair's start point (`pointFaces`).
    point_faces: BTreeMap<usize, Vec<usize>>,
    thinned: Vec<bool>,
}

fn nearest_point_on_line(e0: Vec3, e1: Vec3, p: Vec3) -> Vec3 {
    let e = e1.sub(e0);
    let d = e.length();
    if d < 1e-150 {
        return e0;
    }
    e0.add(e.scale(e.dot(p.sub(e0)) / (d * d)))
}

/// `help::triLineIntersection`: the segment `s → e` against triangle `(a, b, c)`.
fn tri_segment_intersection(a: Vec3, b: Vec3, c: Vec3, s: Vec3, e: Vec3) -> Option<Vec3> {
    const SMALL: f64 = 1e-15;
    let v = s.sub(e);
    let v0 = b.sub(a);
    let v1 = c.sub(a);
    let sp = s.sub(a);
    // Solve [v0 v1 v] (u0, u1, t) = sp.
    let det = v0.dot(v1.cross(v));
    if det.abs() < SMALL {
        return None;
    }
    let t = v0.dot(v1.cross(sp)) / det;
    if !(-SMALL..=1.0 + SMALL).contains(&t) {
        return None;
    }
    let u0 = sp.dot(v1.cross(v)) / det;
    if u0 < -SMALL {
        return None;
    }
    let u1 = v0.dot(sp.cross(v)) / det;
    if u1 < -SMALL || u0 + u1 > 1.0 + SMALL {
        return None;
    }
    Some(s.sub(v.scale(t)))
}

/// `help::lineFaceIntersection`: the segment against the triangle fan of a
/// face about its centre.
fn segment_face_intersection(m: &VolumeMesh, f: usize, s: Vec3, e: Vec3) -> Option<Vec3> {
    let ring = &m.faces[f];
    let c = m.face_centroid(f);
    let k = ring.len();
    (0..k).find_map(|i| tri_segment_intersection(m.points[ring[i]], m.points[ring[(i + 1) % k]], c, s, e))
}

impl Hairs {
    /// `calculateHairEdges` on the one-layer mesh `m`.
    fn new(m: &VolumeMesh, orig: &VolumeMesh, s: &LayerSetup, column: &BTreeMap<usize, Vec<usize>>) -> Self {
        let mut hairs = Vec::new();
        let mut at_point = BTreeMap::new();
        for &wp in &s.wall_pts {
            let col = &column[&wp];
            let kind = match s.kind[&wp] {
                LayerVertex::Patch => INSIDE,
                LayerVertex::Edge => BOUNDARY | ATEDGE,
                LayerVertex::Corner => BOUNDARY | ATCORNER | FEATUREEDGE,
            };
            at_point.insert(col[0], hairs.len());
            hairs.push(Hair { start: col[0], end: col[1], kind });
        }
        let mut point_faces: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for f in m.n_internal_faces()..m.face_count() {
            for &p in &m.faces[f] {
                if at_point.contains_key(&p) {
                    point_faces.entry(p).or_default().push(f);
                }
            }
        }
        // Hairs across side faces: wall-face edges (p, q).
        let mut near_set: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); hairs.len()];
        for (wp, nb) in wall_neighbours(orig, s) {
            let (hi, hj) = (at_point[&column[&wp][0]], at_point[&column[&nb][0]]);
            near_set[hi].insert(hj);
        }
        let near = near_set.into_iter().map(|s| s.into_iter().collect()).collect();
        let n = hairs.len();
        Hairs { hairs, at_point, near, point_faces, thinned: vec![false; n] }
    }

    fn vec(&self, m: &VolumeMesh, h: usize) -> Vec3 {
        m.points[self.hairs[h].end].sub(m.points[self.hairs[h].start])
    }

    /// `calculateHairVectorsAtTheBoundary`.
    fn hair_vectors_at_the_boundary(&self, m: &VolumeMesh) -> Vec<Vec3> {
        let mut out = vec![Vec3::ZERO; self.hairs.len()];
        for (h, hair) in self.hairs.iter().enumerate() {
            if hair.kind & BOUNDARY == 0 {
                continue;
            }
            if hair.kind & FEATUREEDGE != 0 {
                out[h] = self.vec(m, h);
            } else {
                let mut hv = Vec3::ZERO;
                for &f in &self.point_faces[&hair.start] {
                    let ring = &m.faces[f];
                    if !ring.contains(&hair.end) {
                        continue;
                    }
                    let k = ring.len();
                    let pos = ring.iter().position(|&p| p == hair.start).unwrap();
                    let fnormal = m.face_area_vector(f);
                    let ev = if ring[(pos + k - 1) % k] == hair.end {
                        m.points[ring[(pos + 1) % k]].sub(m.points[ring[pos]])
                    } else {
                        m.points[ring[pos]].sub(m.points[ring[(pos + k - 1) % k]])
                    };
                    let hev = fnormal.cross(ev);
                    hv = hv.add(hev.scale(1.0 / (hev.length() + 1e-300)));
                }
                out[h] = hv;
            }
            out[h] = out[h].scale(1.0 / (out[h].length() + 1e-300));
        }
        out
    }

    /// `optimiseHairNormalsAtTheBoundary`.
    fn optimise_normals_at_the_boundary(&self, m: &mut VolumeMesh, c: &BoundaryLayerOptimisationControls) {
        let mut vecs = if c.recalculate_normals {
            self.hair_vectors_at_the_boundary(m)
        } else {
            if c.n_smooth_normals == 0 {
                return;
            }
            (0..self.hairs.len()).map(|h| self.vec(m, h)).collect()
        };
        for _ in 0..c.n_smooth_normals {
            let mut new = vec![Vec3::ZERO; vecs.len()];
            for (h, hair) in self.hairs.iter().enumerate() {
                if hair.kind & BOUNDARY == 0 {
                    continue;
                }
                if hair.kind & (FEATUREEDGE | ATCORNER) != 0 {
                    new[h] = vecs[h];
                } else {
                    // Neighbours whose hair is an edge of a boundary face
                    // holding this hair.
                    let faces: Vec<usize> = self.point_faces[&hair.start].iter().copied().filter(|&f| m.faces[f].contains(&hair.end)).collect();
                    for &j in &self.near[h] {
                        let o = self.hairs[j];
                        let on = faces.iter().any(|&f| face_has_edge(&m.faces[f], o.start, o.end));
                        if on {
                            new[h] = new[h].add(vecs[j]);
                        }
                    }
                }
            }
            for (h, hair) in self.hairs.iter().enumerate() {
                if hair.kind & BOUNDARY != 0 {
                    let n = new[h].scale(1.0 / (new[h].length() + 1e-300));
                    let n = n.add(vecs[h]).scale(0.5);
                    new[h] = n.scale(1.0 / (n.length() + 1e-300));
                }
            }
            vecs = new;
        }
        for (h, hair) in self.hairs.iter().enumerate() {
            if hair.kind & BOUNDARY != 0 {
                let len = self.vec(m, h).length();
                m.points[hair.end] = m.points[hair.start].add(vecs[h].scale(len));
            }
        }
    }

    /// `optimiseHairNormalsInside`.
    fn optimise_normals_inside(&self, m: &mut VolumeMesh, c: &BoundaryLayerOptimisationControls, patch_of: &[usize]) {
        let mut vecs: Vec<Vec3> = (0..self.hairs.len()).map(|h| {
            let v = self.vec(m, h);
            v.scale(1.0 / (v.length() + 1e-300))
        }).collect();
        if c.recalculate_normals {
            // calculateNormalVectors(INSIDE): per patch, Σ S / Σ |S| over the
            // boundary faces at the point; the hair is minus their sum.
            for (h, hair) in self.hairs.iter().enumerate() {
                if hair.kind & INSIDE == 0 {
                    continue;
                }
                let mut per_patch: BTreeMap<usize, (Vec3, f64)> = BTreeMap::new();
                for &f in &self.point_faces[&hair.start] {
                    let s = m.face_area_vector(f);
                    let e = per_patch.entry(patch_of[f]).or_insert((Vec3::ZERO, 0.0));
                    e.0 = e.0.add(s);
                    e.1 += s.length();
                }
                let mut hv = Vec3::ZERO;
                for (n, a) in per_patch.values() {
                    hv = hv.sub(n.scale(1.0 / a));
                }
                vecs[h] = hv.scale(1.0 / (hv.length() + 1e-300));
            }
        } else if c.n_smooth_normals == 0 {
            return;
        }
        for _ in 0..c.n_smooth_normals {
            let mut new = vec![Vec3::ZERO; vecs.len()];
            for (h, hair) in self.hairs.iter().enumerate() {
                if hair.kind & INSIDE == 0 {
                    new[h] = vecs[h];
                } else if hair.kind & ATCORNER != 0 {
                    new[h] = vecs[h];
                } else if hair.kind & ATEDGE != 0 {
                    for &j in &self.near[h] {
                        let o = self.hairs[j];
                        let n = vecs[j].cross(m.points[o.start].sub(m.points[hair.start]));
                        let n = n.scale(1.0 / (n.length() + 1e-300));
                        let nv = vecs[h].sub(n.scale(vecs[h].dot(n)));
                        new[h] = new[h].add(nv.scale(1.0 / (nv.length() + 1e-300)));
                    }
                } else {
                    for &j in &self.near[h] {
                        new[h] = new[h].add(vecs[j]);
                    }
                }
            }
            for (h, hair) in self.hairs.iter().enumerate() {
                if hair.kind & INSIDE != 0 {
                    new[h] = new[h].scale(1.0 / (new[h].length() + 1e-300));
                }
            }
            vecs = new;
        }
        for (h, hair) in self.hairs.iter().enumerate() {
            if hair.kind & INSIDE != 0 {
                let len = self.vec(m, h).length();
                m.points[hair.end] = m.points[hair.start].add(vecs[h].scale(len));
            }
        }
    }

    /// `calculateThickness(heI, heJ)`.
    fn thickness_against(&self, m: &VolumeMesh, hi: usize, hj: usize, c: &BoundaryLayerOptimisationControls) -> f64 {
        let (he, nhe) = (self.hairs[hi], self.hairs[hj]);
        let sp = m.points[he.start];
        let ep = m.points[nhe.start];
        let dv = ep.sub(sp);
        let mag_dv = dv.length();
        let curr = self.vec(m, hi).length();
        let mut ret = curr;
        let curr_nei = self.vec(m, hj).length();
        let mut sugg_nei = curr_nei;
        let np_alpha = nearest_point_on_line(sp, ep, m.points[he.end]);
        let curr_height = np_alpha.sub(m.points[he.end]).length();
        let mut ret_height = curr_height;
        let sgn = |x: f64| if x >= 0.0 { 1.0 } else { -1.0 };
        let cos_alpha = sgn(np_alpha.sub(sp).dot(dv)) * np_alpha.sub(sp).length();
        let alpha = (cos_alpha / (curr + 1e-300)).clamp(-1.0, 1.0).r_acos();
        let np_beta = nearest_point_on_line(ep, sp, m.points[nhe.end]);
        let curr_nei_height = np_beta.sub(m.points[nhe.end]).length();
        let mut sugg_nei_height = curr_nei_height;
        let cos_beta = sgn(np_beta.sub(ep).dot(dv.scale(-1.0))) * np_beta.sub(ep).length();
        let beta = (cos_beta / (curr_nei + 1e-300)).clamp(-1.0, 1.0).r_acos();
        let pi = core::f64::consts::PI;
        if alpha + beta < pi {
            let gamma = pi - (alpha + beta);
            let sin_gamma = gamma.r_sin().max(1e-15);
            let sin_alpha = alpha.r_sin().max(1e-15);
            let sin_beta = beta.r_sin().max(1e-15);
            ret = ret.min(c.feature_size_factor * mag_dv * sin_beta / sin_gamma);
            ret_height *= ret / (curr + 1e-300);
            sugg_nei = sugg_nei.min(c.feature_size_factor * mag_dv * sin_alpha / sin_gamma);
            sugg_nei_height *= sugg_nei / (curr_nei + 1e-300);
        }
        let tan_val = (ret_height - sugg_nei_height) / (mag_dv + 1e-300);
        if tan_val > c.rel_thickness_tol {
            ret_height = sugg_nei_height + c.rel_thickness_tol * mag_dv;
            ret = (ret_height / curr_height) * curr;
        }
        ret
    }

    /// `calculateThicknessOverCell(heI, cellI, baseFaceI)`.
    fn thickness_over_cell(&self, m: &VolumeMesh, h: usize, cell_faces: &[usize], base: usize, c: &BoundaryLayerOptimisationControls) -> f64 {
        let he = self.hairs[h];
        let (sp, ep) = (m.points[he.start], m.points[he.end]);
        let mut max_t = self.vec(m, h).length();
        if face_has_edge(&m.faces[base], he.start, he.end) {
            return max_t;
        }
        for &f in cell_faces {
            if f == base {
                continue;
            }
            if share_an_edge(&m.faces[base], &m.faces[f]) && !m.faces[f].contains(&he.start) {
                if let Some(x) = segment_face_intersection(m, f, sp, ep) {
                    max_t = max_t.min(c.feature_size_factor * x.sub(sp).length());
                }
            }
        }
        max_t
    }

    /// `optimiseThicknessVariation(edgeType)`.
    fn optimise_thickness_variation(&mut self, m: &mut VolumeMesh, edge_type: u8, cf: &[Vec<usize>], c: &BoundaryLayerOptimisationControls) {
        let n = self.hairs.len();
        let dirs: Vec<Vec3> = (0..n).map(|h| {
            let v = self.vec(m, h);
            v.scale(1.0 / (v.length() + 1e-300))
        }).collect();
        let mut length: Vec<f64> = (0..n).map(|h| self.vec(m, h).length() + 1e-300).collect();
        let mut active = vec![true; n];
        for _ in 0..1000 {
            let mut changed = false;
            let mut influenced = vec![false; n];
            for h in 0..n {
                if self.hairs[h].kind & edge_type == 0 || !active[h] {
                    continue;
                }
                let he = self.hairs[h];
                let mut max_t = length[h];
                let mut influencers = Vec::new();
                for &bf in &self.point_faces[&he.start] {
                    if m.faces[bf].contains(&he.end) {
                        continue;
                    }
                    let own = m.owner[bf];
                    max_t = max_t.min(self.thickness_over_cell(m, h, &cf[own], bf, c));
                    for &p in &m.faces[bf] {
                        let Some(&j) = self.at_point.get(&p) else { continue };
                        if j == h {
                            continue;
                        }
                        influencers.push(j);
                        max_t = max_t.min(self.thickness_against(m, h, j, c));
                    }
                }
                if length[h] > max_t {
                    length[h] = max_t;
                    changed = true;
                    self.thinned[h] = true;
                    for j in influencers {
                        influenced[j] = true;
                    }
                }
            }
            if !changed {
                break;
            }
            for h in 0..n {
                if self.hairs[h].kind & edge_type != 0 {
                    let s = m.points[self.hairs[h].start];
                    m.points[self.hairs[h].end] = s.add(dirs[h].scale(length[h]));
                }
            }
            active = influenced;
        }
    }

    /// `optimiseLayersAtExittingFaces`: true when a thinned hair ends at a
    /// point with more than two hairs ending there.
    fn layers_at_exiting_faces_need_work(&self) -> bool {
        let mut ending: BTreeMap<usize, usize> = BTreeMap::new();
        for h in &self.hairs {
            *ending.entry(h.end).or_insert(0) += 1;
        }
        self.hairs.iter().zip(&self.thinned).any(|(h, &t)| t && ending[&h.end] > 2)
    }
}

fn face_has_edge(ring: &[usize], a: usize, b: usize) -> bool {
    let k = ring.len();
    (0..k).any(|i| {
        let (p, q) = (ring[i], ring[(i + 1) % k]);
        (p == a && q == b) || (p == b && q == a)
    })
}

fn share_an_edge(f: &[usize], g: &[usize]) -> bool {
    let k = f.len();
    (0..k).any(|i| face_has_edge(g, f[i], f[(i + 1) % k]))
}

/// Wall-point pairs joined by a wall-face edge (both directions).
fn wall_neighbours(orig: &VolumeMesh, s: &LayerSetup) -> Vec<(usize, usize)> {
    let mut out = BTreeSet::new();
    for &wf in &s.wall_faces {
        let f = &orig.faces[wf];
        let k = f.len();
        for i in 0..k {
            let (a, b) = (f[i], f[(i + 1) % k]);
            out.insert((a, b));
            out.insert((b, a));
        }
    }
    out.into_iter().collect()
}

/// One optimiser call's profile, for the notes (gh:#1241).
fn profile(r: &crate::mesh_optimizer::OptimiserReport) -> String {
    format!(
        "{} passes in {} global, {} nodes smoothed, {:.1} s smoothing / {:.1} s regions / {:.1} s checks",
        r.iterations, r.global_iterations, r.nodes_smoothed, r.seconds_smooth, r.seconds_build, r.seconds_check
    )
}

/// `boundaryLayerOptimisation::optimiseLayer`.
fn optimise_layer(m: &mut VolumeMesh, hairs: &mut Hairs, c: &BoundaryLayerOptimisationControls, patch_of: &[usize]) -> usize {
    let cf = crate::mesh_optimizer::cell_faces(m);
    let mut n_iter = 0;
    loop {
        hairs.thinned.iter_mut().for_each(|t| *t = false);
        hairs.optimise_normals_at_the_boundary(m, c);
        hairs.optimise_thickness_variation(m, BOUNDARY, &cf, c);
        hairs.optimise_normals_inside(m, c, patch_of);
        hairs.optimise_thickness_variation(m, INSIDE, &cf, c);
        n_iter += 1;
        if !(hairs.layers_at_exiting_faces_need_work() && n_iter < c.max_num_iterations) {
            break;
        }
    }
    n_iter
}

/// [`LayerQualityStrategy::CfMeshOptimise`]: grow `n_layers` layers on
/// `layer_patches` the cfMesh way (see the module docs for the steps and
/// what is not ported). Returns the mesh and what happened; the mesh is the
/// input rebuilt with no layer when [`crate::layers`] cannot set up the layer
/// (the same cases as the back-off).
pub fn add_boundary_layers_cfmesh(
    mesh: &VolumeMesh,
    layer_patches: &[&str],
    n_layers: usize,
    first_thickness: f64,
    expansion: f64,
    controls: &BoundaryLayerOptimisationControls,
) -> (VolumeMesh, LayerOutcome) {
    let strategy = LayerQualityStrategy::CfMeshOptimise;
    let Some(s) = layer_setup(mesh, layer_patches, n_layers, first_thickness, expansion) else {
        return (from_cell_faces(mesh.points.clone(), &cells_faces(mesh)), LayerOutcome::none(strategy, n_layers));
    };
    let mut notes = Vec::new();
    // 1. One layer at the cap.
    let ones: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&p| (p, 1)).collect();
    let d1 = [0.0, s.d[n_layers]];
    let (pts, column) = layer_columns(mesh, &s.wall_pts, &s.inward, &d1, &s.cap, &ones);
    let mut m1 = assemble_layered(mesh, &s.wall_faces, &s.wall_pts, pts, &column, &ones);
    // Patch of each boundary face of the one-layer mesh: layer faces (the
    // prisms' wall faces) are patch 0, everything else patch 1 (only the
    // grouping matters to `calculateNormalVectors`).
    let n_prism0 = mesh.n_cells;
    let wall_nodes: BTreeSet<usize> = column.values().map(|c| c[0]).collect();
    let patch_of: Vec<usize> = (0..m1.face_count())
        .map(|f| {
            let on_wall = m1.neighbour[f].is_none() && m1.owner[f] >= n_prism0 && m1.faces[f].iter().all(|p| wall_nodes.contains(p));
            if on_wall {
                0
            } else {
                1
            }
        })
        .collect();
    let mut hairs = Hairs::new(&m1, mesh, &s, &column);
    let none = vec![false; m1.points.len()];

    // 2. tetMeshGenerator::optimiseFinalMesh.
    let stall = controls.stop_untangling_on_stall;
    let r = untangle_mesh_fv_bounded(&mut m1, &none, 10, 50, 2, false, stall);
    notes.push(format!("optimizeMeshFV/untangleMeshFV: bad faces {} -> {}; {}", r.flagged_before, r.flagged_after, profile(&r)));
    let r = optimize_low_quality_faces(&mut m1, &none, 10);
    notes.push(format!("optimizeLowQualityFaces: low-quality faces {} -> {} ({} passes); {}", r.flagged_before, r.flagged_after, r.iterations, profile(&r)));
    let passes = optimise_layer(&mut m1, &mut hairs, controls, &patch_of);
    let n_thinned = hairs.thinned.iter().filter(|&&t| t).count();
    notes.push(format!("optimiseLayer: {passes} pass(es), {n_thinned} hairs thinned in the last"));
    let mut layer_locked = vec![false; m1.points.len()];
    for f in 0..m1.face_count() {
        if m1.owner[f] >= n_prism0 || m1.neighbour[f].is_some_and(|n| n >= n_prism0) {
            for &p in &m1.faces[f] {
                layer_locked[p] = true;
            }
        }
    }
    let r1 = untangle_mesh_fv(&mut m1, &layer_locked, 1, 50, 0, false);
    let r2 = untangle_mesh_fv(&mut m1, &layer_locked, 2, 50, 0, false);
    notes.push(format!("optimizeBoundaryLayer untangle (layer locked): bad faces {} -> {}; {}; {}", r1.flagged_before, r2.flagged_after, profile(&r1), profile(&r2)));
    let r = untangle_mesh_fv_bounded(&mut m1, &none, 10, 50, 2, false, stall);
    let (_, at_bnd) = bad_faces_at_the_boundary(&m1);
    notes.push(format!("final untangleMeshFV: bad faces {} -> {} ({at_bnd} touch a boundary point); {}", r.flagged_before, r.flagged_after, profile(&r)));

    // 3. refBoundaryLayers: split each hair by the grading.
    let mut moved = mesh.clone();
    let mut k = 0usize;
    for (op, p) in moved.points.iter_mut().enumerate() {
        if !s.wall_pts.contains(&op) {
            *p = m1.points[k];
            k += 1;
        }
    }
    let counts: BTreeMap<usize, usize> = s.wall_pts.iter().map(|&p| (p, n_layers)).collect();
    let mut totals: BTreeMap<usize, f64> = BTreeMap::new();
    let mut new_points: Vec<Vec3> = (0..moved.points.len()).filter(|op| !s.wall_pts.contains(op)).map(|op| moved.points[op]).collect();
    let mut column_n: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &wp in &s.wall_pts {
        let (a, b) = (m1.points[column[&wp][0]], m1.points[column[&wp][1]]);
        totals.insert(wp, b.sub(a).length());
        let mut col = Vec::with_capacity(n_layers + 1);
        for j in 0..=n_layers {
            col.push(new_points.len());
            new_points.push(a.add(b.sub(a).scale(s.d[j] / s.d[n_layers])));
        }
        column_n.insert(wp, col);
    }
    let mut mn = assemble_layered(&moved, &s.wall_faces, &s.wall_pts, new_points, &column_n, &counts);

    // 4. untangleBoundaryLayer.
    let mut in_layer = vec![false; mn.points.len()];
    for f in 0..mn.face_count() {
        if mn.owner[f] >= n_prism0 || mn.neighbour[f].is_some_and(|n| n >= n_prism0) {
            for &p in &mn.faces[f] {
                in_layer[p] = true;
            }
        }
    }
    let r = optimize_low_quality_faces(&mut mn, &in_layer, 10);
    notes.push(format!("untangleBoundaryLayer optimizeLowQualityFaces (layer locked): {} -> {}; {}", r.flagged_before, r.flagged_after, profile(&r)));
    let none_n = vec![false; mn.points.len()];
    let r = untangle_mesh_fv(&mut mn, &none_n, 2, 50, 1, true);
    notes.push(format!("untangleBoundaryLayer untangleMeshFV (relaxed): {} -> {}; {}", r.flagged_before, r.flagged_after, profile(&r)));

    // The hairs after the last untangle (the split points may have moved).
    for &wp in &s.wall_pts {
        let col = &column_n[&wp];
        totals.insert(wp, mn.points[col[n_layers]].sub(mn.points[col[0]]).length());
    }
    let mut outcome = LayerOutcome::from_columns(strategy, mesh, &s, &counts, &totals, passes);
    outcome.remaining_bad_faces = find_bad_faces(&mn).iter().filter(|&&b| b).count();
    let low = find_low_quality_faces(&mn).iter().filter(|&&b| b).count();
    notes.push(format!("at the end: {} bad faces (findBadFaces), {low} low-quality faces (findLowQualityFaces)", outcome.remaining_bad_faces));
    outcome.notes = notes;
    (mn, outcome)
}
