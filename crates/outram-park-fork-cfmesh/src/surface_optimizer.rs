// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh v1.1 (github.com/DAFoam/cfmesh, commit e944f6ba695f),
// the serial parts only:
//   meshLibrary/utilities/meshes/partTriMesh/
//     partTriMesh.{C,H}           (constructors, updateVertex, updateVertices)
//     partTriMeshAddressing.C     (createPointsAndTrias)
//     partTriMeshSimplex.{C,H}
//   meshLibrary/utilities/smoothers/geometry/meshSurfaceOptimizer/
//     advancedSurfaceSmoothers/surfaceOptimizer/surfaceOptimizer.{C,H}
//   meshLibrary/utilities/helperClasses/matrices/matrix2DI.H (the 2x2 solve)
//   meshLibrary/utilities/surfaceTools/meshSurfaceCheckInvertedVertices/
//     meshSurfaceCheckInvertedVertices.{C,H}
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (isFaceConvexAndOk)
// Not ported: partTriMeshParallelAddressing.C and every `Pstream::parRun()`
// branch (MPI, buffer layers).
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **Surface smoothing tools** (gh:#1258): the pieces cfMesh's
//! `meshSurfaceOptimizer` (gh:#1259) is built from.
//!
//! - [`PartTriMesh`]: the mesh surface as triangles. A polygon of more than
//!   three points gets a point at its centre and a triangle per side to it;
//!   every face also gets one "shape" triangle per corner (the corner and
//!   its two neighbours) that only that corner's simplex uses.
//! - [`PartTriMeshSimplex`]: the triangles around one point, the point
//!   first in each.
//! - [`optimize_point`]: cfMesh's `surfaceOptimizer`, the in-plane analogue
//!   of the volume optimiser: minimise `Σ L²/A` over the simplex's
//!   triangles (`L²` the two squared sides at the free point, `A` the
//!   signed area, stabilised near zero), by divide and conquer, then
//!   Newton steps, keeping the better.
//! - [`check_inverted_vertices`]: cfMesh's `meshSurfaceCheckInvertedVertices`:
//!   a boundary point is inverted when a triangle it makes with a face
//!   centre faces away from its normal (its patch's normal on a feature
//!   edge), is degenerate, or turns against its partner, or when it is a
//!   reflex corner of a face.
//!
//! The geometry conventions are [`crate::surface_engine`]'s (area-vector
//! face normals, OpenFOAM face centres). Constants are OpenFOAM's for
//! `double`: `VSMALL` 1e-300, `ROOTVSMALL` 1e-150, `SMALL` 1e-15,
//! `VGREAT` 1e300.

use crate::math::Vec3;
use crate::surface_engine::{ring_area_vector, SurfaceEngine, SurfacePartitioner, VSMALL};
use std::collections::{BTreeMap, BTreeSet};

const ROOTVSMALL: f64 = 1.0e-150;
const SMALL: f64 = 1.0e-15;
const VGREAT: f64 = 1.0e300;

/// `partTriMesh::vertexTypes`: none.
pub const NONE: u8 = 0;
/// A point of the mesh surface, free to smooth.
pub const SMOOTH: u8 = 1;
/// An auxiliary point at a polygon's centre.
pub const FACECENTRE: u8 = 2;
/// A feature-edge point (two feature edges).
pub const FEATUREEDGE: u8 = 32;
/// A corner (more than two feature edges).
pub const CORNER: u8 = 64;

/// Marks "no such point" in the label maps (upstream's `-1`).
pub const NO_POINT: usize = usize::MAX;

/// cfMesh `partTriMesh` (serial): the triangulated mesh surface. See the
/// module documentation.
#[derive(Debug, Clone)]
pub struct PartTriMesh {
    /// Triangulation points.
    pub points: Vec<Vec3>,
    /// Triangles (triangulation labels) and the patch of the face they
    /// came from.
    pub triangles: Vec<([usize; 3], usize)>,
    /// Triangles at each point, ascending (`pointTriangles`).
    pub point_triangles: Vec<Vec<usize>>,
    /// [`SMOOTH`] / [`FACECENTRE`] / [`FEATUREEDGE`] / [`CORNER`] flags.
    pub point_type: Vec<u8>,
    /// Boundary point of each triangulation point, [`NO_POINT`] for a face
    /// centre (`pointLabelInMeshSurface`).
    pub point_label_in_mesh_surface: Vec<usize>,
    /// Triangulation point of each boundary point, [`NO_POINT`] when the
    /// point is not used (`meshSurfacePointLabelInTriMesh`).
    pub mesh_surface_point_label_in_tri_mesh: Vec<usize>,
}

/// The area-weighted centre of the triangles around face-centre point `c`
/// (`partTriMesh::updateVertex` / `updateVertices`).
fn centre_of(points: &[Vec3], triangles: &[([usize; 3], usize)], tri_ids: &[usize]) -> Vec3 {
    let (mut centre, mut area_sum) = (Vec3::ZERO, 0.0);
    for &t in tri_ids {
        let tri = triangles[t].0;
        let c = points[tri[0]].add(points[tri[1]]).add(points[tri[2]]).scale(1.0 / 3.0);
        let area = 0.5 * points[tri[1]].sub(points[tri[0]]).cross(points[tri[2]].sub(points[tri[0]])).length() + VSMALL;
        centre = centre.add(c.scale(area));
        area_sum += area;
    }
    centre.scale(1.0 / area_sum)
}

impl PartTriMesh {
    /// Every boundary face (`partTriMesh(mPart)`). `engine`'s geometry
    /// must be current for `points`.
    pub fn new(engine: &SurfaceEngine, part: &SurfacePartitioner, points: &[Vec3]) -> Self {
        Self::create(engine, part, points, &vec![1u8; engine.n_boundary_faces()])
    }

    /// The faces at the `inverted` points (mesh labels) and
    /// `additional_layers` rings of faces around them
    /// (`partTriMesh(mPart, invertedPoints, additionalLayers)`).
    pub fn around_points(engine: &SurfaceEngine, part: &SurfacePartitioner, points: &[Vec3], inverted: &BTreeSet<usize>, additional_layers: u8) -> Self {
        let mut use_face = vec![0u8; engine.n_boundary_faces()];
        for (b, pf) in engine.point_faces.iter().enumerate() {
            if inverted.contains(&engine.boundary_points[b]) {
                for &bf in pf {
                    use_face[bf] = 1;
                }
            }
        }
        for layer in 1..additional_layers.saturating_add(1) {
            for bf in 0..use_face.len() {
                if use_face[bf] == layer {
                    for &v in &engine.face_rings[bf] {
                        for &g in &engine.point_faces[engine.bp[v]] {
                            if use_face[g] == 0 {
                                use_face[g] = layer + 1;
                            }
                        }
                    }
                }
            }
        }
        Self::create(engine, part, points, &use_face)
    }

    /// `createPointsAndTrias`.
    fn create(engine: &SurfaceEngine, part: &SurfacePartitioner, points: &[Vec3], use_face: &[u8]) -> Self {
        let nbp = engine.n_boundary_points();
        let mut in_tri = vec![NO_POINT; nbp];
        let mut node_for_face = vec![NO_POINT; engine.n_boundary_faces()];
        let mut n = 0usize;
        let mut triangles: Vec<([usize; 3], usize)> = Vec::new();
        for bf in 0..engine.n_boundary_faces() {
            if use_face[bf] == 0 {
                continue;
            }
            let ring = &engine.face_rings[bf];
            let k = ring.len();
            if k > 3 {
                node_for_face[bf] = n;
                n += 1;
            }
            for &v in ring {
                let b = engine.bp[v];
                if in_tri[b] == NO_POINT {
                    in_tri[b] = n;
                    n += 1;
                }
            }
            let patch = part.face_patches[bf];
            let t = |v: usize| in_tri[engine.bp[v]];
            if k > 3 {
                for e in 0..k {
                    triangles.push(([t(ring[e]), t(ring[(e + 1) % k]), node_for_face[bf]], patch));
                    triangles.push(([t(ring[e]), t(ring[(e + 1) % k]), t(ring[(e + k - 1) % k])], patch));
                }
            } else {
                triangles.push(([t(ring[0]), t(ring[1]), t(ring[2])], patch));
                for e in 0..k {
                    triangles.push(([t(ring[e]), t(ring[(e + 1) % k]), t(ring[(e + k - 1) % k])], patch));
                }
            }
        }
        let mut tri_points = vec![Vec3::ZERO; n];
        let mut point_type = vec![NONE; n];
        let mut in_surface = vec![NO_POINT; n];
        for b in 0..nbp {
            let p = in_tri[b];
            if p != NO_POINT {
                in_surface[p] = b;
                tri_points[p] = points[engine.boundary_points[b]];
                point_type[p] |= SMOOTH;
            }
        }
        for bf in 0..engine.n_boundary_faces() {
            let p = node_for_face[bf];
            if p != NO_POINT {
                tri_points[p] = engine.face_centres[bf];
                point_type[p] = FACECENTRE;
            }
        }
        for &b in &part.corners {
            if in_tri[b] != NO_POINT {
                point_type[in_tri[b]] |= CORNER;
            }
        }
        for &b in &part.edge_points {
            if in_tri[b] != NO_POINT {
                point_type[in_tri[b]] |= FEATUREEDGE;
            }
        }
        let mut point_triangles = vec![Vec::new(); n];
        for (i, (t, _)) in triangles.iter().enumerate() {
            for &p in t {
                point_triangles[p].push(i);
            }
        }
        PartTriMesh {
            points: tri_points,
            triangles,
            point_triangles,
            point_type,
            point_label_in_mesh_surface: in_surface,
            mesh_surface_point_label_in_tri_mesh: in_tri,
        }
    }

    /// `updateVertex`: move triangulation point `p` and recentre the face
    /// centres around it. Moving a face centre itself has no effect on the
    /// mesh (upstream warns and returns).
    pub fn update_vertex(&mut self, p: usize, new_p: Vec3) {
        self.points[p] = new_p;
        if self.point_type[p] & FACECENTRE != 0 {
            return;
        }
        let mut centres: Vec<usize> = Vec::new();
        for &t in &self.point_triangles[p] {
            let c = self.triangles[t].0[2];
            if self.point_type[c] & FACECENTRE != 0 && !centres.contains(&c) {
                centres.push(c);
            }
        }
        for c in centres {
            self.points[c] = centre_of(&self.points, &self.triangles, &self.point_triangles[c]);
        }
    }

    /// `updateVertices(movedPoints)`: copy the mesh coordinates of the
    /// boundary points `moved` and recentre the face centres around them.
    pub fn update_vertices(&mut self, engine: &SurfaceEngine, points: &[Vec3], moved: &[usize]) {
        let mut update = vec![NONE; self.points.len()];
        for &b in moved {
            let p = self.mesh_surface_point_label_in_tri_mesh[b];
            if p == NO_POINT {
                continue;
            }
            self.points[p] = points[engine.boundary_points[b]];
            update[p] |= SMOOTH;
            for &t in &self.point_triangles[p] {
                let c = self.triangles[t].0[2];
                if self.point_type[c] & FACECENTRE != 0 {
                    update[c] |= FACECENTRE;
                }
            }
        }
        for c in 0..self.points.len() {
            if update[c] & FACECENTRE != 0 {
                self.points[c] = centre_of(&self.points, &self.triangles, &self.point_triangles[c]);
            }
        }
    }

    /// `updateVertices()`: every boundary point.
    pub fn update_all_vertices(&mut self, engine: &SurfaceEngine, points: &[Vec3]) {
        let all: Vec<usize> = (0..engine.n_boundary_points()).collect();
        self.update_vertices(engine, points, &all);
    }
}

/// cfMesh `partTriMeshSimplex`: the points and triangles around one
/// triangulation point, that point first in each triangle.
#[derive(Debug, Clone)]
pub struct PartTriMeshSimplex {
    /// Points, in the order the triangles first reach them.
    pub pts: Vec<Vec3>,
    /// Triangles (indices into `pts`), the centre point first.
    pub trias: Vec<[usize; 3]>,
}

impl PartTriMeshSimplex {
    /// The simplex of point `p`, `None` where upstream stops with a fatal
    /// error (no triangle usable by `p`). A triangle whose third point is
    /// not a face centre is a shape triangle and serves only its first
    /// point.
    pub fn new(tm: &PartTriMesh, p: usize) -> Option<Self> {
        let mut addr: BTreeMap<usize, usize> = BTreeMap::new();
        let mut pts = Vec::new();
        let mut trias = Vec::new();
        for &t in &tm.point_triangles[p] {
            let tri = tm.triangles[t].0;
            for &q in &tri {
                if let std::collections::btree_map::Entry::Vacant(e) = addr.entry(q) {
                    e.insert(pts.len());
                    pts.push(tm.points[q]);
                }
            }
            let pos = tri.iter().position(|&q| q == p)?;
            if tm.point_type[tri[2]] & FACECENTRE == 0 && pos != 0 {
                continue;
            }
            trias.push([addr[&tri[pos]], addr[&tri[(pos + 1) % 3]], addr[&tri[(pos + 2) % 3]]]);
        }
        (!pts.is_empty() && !trias.is_empty()).then_some(PartTriMeshSimplex { pts, trias })
    }

    /// The simplex's centre point.
    pub fn centre_point(&self) -> Vec3 {
        self.pts[self.trias[0][0]]
    }

    /// `partTriMeshSimplex::normal`: the sum of the triangles' area vectors
    /// over the sum of their magnitudes.
    pub fn normal(&self) -> Vec3 {
        let (mut n, mut mag) = (Vec3::ZERO, 0.0);
        for t in &self.trias {
            let a = self.pts[t[1]].sub(self.pts[t[0]]).cross(self.pts[t[2]].sub(self.pts[t[0]])).scale(0.5);
            mag += a.length();
            n = n.add(a);
        }
        n.scale(1.0 / (mag + VSMALL))
    }
}

/// The signed area of `t` in the x-y plane.
fn area_xy(p0: Vec3, p1: Vec3, p2: Vec3) -> f64 {
    0.5 * ((p1.x - p0.x) * (p2.y - p0.y) - (p2.x - p0.x) * (p1.y - p0.y))
}

fn mag_sqr(v: Vec3) -> f64 {
    v.dot(v)
}

/// `surfaceOptimizer::evaluateStabilisationFactor`.
fn stabilisation_factor(pts: &[Vec3], trias: &[[usize; 3]]) -> f64 {
    let (mut a_min, mut lsq_max) = (VGREAT, 0.0f64);
    for t in trias {
        let (p0, p1, p2) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        a_min = a_min.min(area_xy(p0, p1, p2));
        lsq_max = lsq_max.max(mag_sqr(p0.sub(p1)) + mag_sqr(p2.sub(p0)));
    }
    if a_min < SMALL * lsq_max {
        SMALL * lsq_max
    } else {
        0.0
    }
}

/// `surfaceOptimizer::evaluateFunc`.
fn evaluate_func(pts: &[Vec3], trias: &[[usize; 3]], k: f64) -> f64 {
    let mut func = 0.0;
    for t in trias {
        let (p0, p1, p2) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        let a = area_xy(p0, p1, p2);
        let stab = (a * a + k).sqrt();
        let lsq = mag_sqr(p0.sub(p1)) + mag_sqr(p2.sub(p0));
        func += lsq / (0.5 * (a + stab)).max(VSMALL);
    }
    func
}

/// A symmetric-or-not 2x2 tensor, row-major.
type T2 = [[f64; 2]; 2];

/// `surfaceOptimizer::evaluateGradients` (the x-y block; the z row and
/// column are zero there).
fn evaluate_gradients(pts: &[Vec3], trias: &[[usize; 3]], k: f64) -> ([f64; 2], T2) {
    let mut g = [0.0; 2];
    let mut h: T2 = [[0.0; 2]; 2];
    let hl: T2 = [[4.0, 0.0], [0.0, 4.0]];
    for t in trias {
        let (p0, p1, p2) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        if mag_sqr(p1.sub(p2)) < VSMALL {
            continue;
        }
        let lsq = mag_sqr(p0.sub(p1)) + mag_sqr(p2.sub(p0));
        let a = area_xy(p0, p1, p2);
        let stab = (a * a + k).sqrt();
        let astab = (0.5 * (a + stab)).max(ROOTVSMALL);
        let ga = [0.5 * (p1.y - p2.y), 0.5 * (p2.x - p1.x)];
        let gas = [0.5 * (ga[0] + a * ga[0] / stab), 0.5 * (ga[1] + a * ga[1] / stab)];
        let mut hgas: T2 = [[0.0; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                hgas[i][j] = 0.5 * ((ga[i] * ga[j]) / stab - a * a * (ga[i] * ga[j]) / stab.powi(3));
            }
        }
        let gl = [4.0 * p0.x - 2.0 * p1.x - 2.0 * p2.x, 4.0 * p0.y - 2.0 * p1.y - 2.0 * p2.y];
        let sq = astab * astab;
        for i in 0..2 {
            g[i] += gl[i] / astab - (lsq * gas[i]) / sq;
        }
        for i in 0..2 {
            for j in 0..2 {
                // twoSymm(gradLt * gradAstab) = outer + its transpose.
                let two_symm = gl[i] * gas[j] + gl[j] * gas[i];
                h[i][j] += hl[i][j] / astab - two_symm / sq - hgas[i][j] * lsq / sq + 2.0 * lsq * (gas[i] * gas[j]) / (sq * astab);
            }
        }
    }
    if h[0][0].abs() < VSMALL {
        h[0][0] = VSMALL;
    }
    if h[1][1].abs() < VSMALL {
        h[1][1] = VSMALL;
    }
    (g, h)
}

/// `surfaceOptimizer::dirVecs`.
const DIR_VECS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]];

/// `surfaceOptimizer::optimiseDivideAndConquer`.
fn divide_and_conquer(pts: &mut [Vec3], trias: &[[usize; 3]], p_min: Vec3, p_max: Vec3, tol: f64) -> f64 {
    let c = trias[0][0];
    pts[c] = p_max.add(p_min).scale(0.5);
    let mut curr = pts[c];
    let (mut dx, mut dy) = ((p_max.x - p_min.x) / 2.0, (p_max.y - p_min.y) / 2.0);
    let mut k = stabilisation_factor(pts, trias);
    let mut func_after = evaluate_func(pts, trias, k);
    for _ in 0..100 {
        let func_before = func_after;
        func_after = VGREAT;
        let mut min_centre = Vec3::ZERO;
        for d in DIR_VECS {
            pts[c].x = curr.x + 0.5 * d[0] * dx;
            pts[c].y = curr.y + 0.5 * d[1] * dy;
            k = stabilisation_factor(pts, trias);
            let f = evaluate_func(pts, trias, k);
            if f < func_after {
                min_centre = pts[c];
                func_after = f;
            }
        }
        curr = min_centre;
        pts[c] = min_centre;
        dx *= 0.5;
        dy *= 0.5;
        if (func_after - func_before).abs() / func_after < tol {
            break;
        }
    }
    func_after
}

/// `surfaceOptimizer::optimiseSteepestDescent` (Newton steps on the 2x2
/// Hessian, `matrix2D::solveFirst` / `solveSecond`, each capped at 0.2 of
/// the bounding-box diagonal).
fn steepest_descent(pts: &mut [Vec3], trias: &[[usize; 3]], p_min: Vec3, p_max: Vec3, tol: f64) -> f64 {
    let c = trias[0][0];
    let avg_edge = p_max.sub(p_min).length();
    let mut k = stabilisation_factor(pts, trias);
    let mut func_after = evaluate_func(pts, trias, k);
    for _ in 0..100u8 {
        let func_before = func_after;
        let (g, h) = evaluate_gradients(pts, trias, k);
        let det = h[0][0] * h[1][1] - h[0][1] * h[1][0];
        let mut disp = Vec3::ZERO;
        if det.abs() >= VSMALL {
            disp.x = (h[1][1] * g[0] - h[0][1] * g[1]) / det;
            disp.y = (-h[1][0] * g[0] + h[0][0] * g[1]) / det;
            let m = disp.length();
            if m > 0.2 * avg_edge {
                disp = disp.scale(0.2 * avg_edge / m);
            }
        }
        pts[c] = pts[c].sub(disp);
        k = stabilisation_factor(pts, trias);
        func_after = evaluate_func(pts, trias, k);
        if (func_after - func_before).abs() / func_before < tol {
            break;
        }
    }
    func_after
}

/// cfMesh `surfaceOptimizer::optimizePoint`: the optimised position of the
/// simplex's centre point `pts[trias[0][0]]`, the points lying in the x-y
/// plane (z is ignored). `pts` is left with the centre point moved there.
/// The result is NaN when the simplex has no extent (upstream divides by
/// it too; [`crate::surface_optimizer`]'s callers then keep the point).
pub fn optimize_point(pts: &mut [Vec3], trias: &[[usize; 3]], tol: f64) -> Vec3 {
    let mut p_min = pts[trias[0][1]];
    let mut p_max = p_min;
    for t in trias {
        for &i in &t[1..3] {
            let q = pts[i];
            p_min = Vec3::new(p_min.x.min(q.x), p_min.y.min(q.y), p_min.z.min(q.z));
            p_max = Vec3::new(p_max.x.max(q.x), p_max.y.max(q.y), p_max.z.max(q.z));
        }
    }
    let scale = p_max.sub(p_min).length();
    for p in pts.iter_mut() {
        *p = p.scale(1.0 / scale);
    }
    let (p_min, p_max) = (p_min.scale(1.0 / scale), p_max.scale(1.0 / scale));
    let c = trias[0][0];
    let func_divide = divide_and_conquer(pts, trias, p_min, p_max, tol);
    let new_point = pts[c];
    let func_steepest = steepest_descent(pts, trias, p_min, p_max, tol);
    if func_steepest > func_divide {
        pts[c] = new_point;
    }
    for p in pts.iter_mut() {
        *p = p.scale(scale);
    }
    pts[c]
}

/// `help::isFaceConvexAndOk`: whether each corner of `ring` turns with the
/// face normal (`(e_prev × e_next)·n ≥ −0.05`, unit edge vectors); every
/// corner fails on a face of zero area. Upstream projects the points onto
/// the face plane and then does not use the projection; neither does this.
pub fn is_face_convex_and_ok(points: &[Vec3], ring: &[usize]) -> (bool, Vec<bool>) {
    let normal = ring_area_vector(points, ring);
    let mag = normal.length();
    if mag < VSMALL {
        return (false, vec![false; ring.len()]);
    }
    let normal = normal.scale(1.0 / (mag + VSMALL));
    let k = ring.len();
    let edge: Vec<Vec3> = (0..k)
        .map(|i| {
            let v = points[ring[(i + 1) % k]].sub(points[ring[i]]);
            v.scale(1.0 / (v.length() + VSMALL))
        })
        .collect();
    let ok: Vec<bool> = (0..k).map(|i| edge[(i + k - 1) % k].cross(edge[i]).dot(normal) >= -0.05).collect();
    (ok.iter().all(|&o| o), ok)
}

/// cfMesh `meshSurfaceCheckInvertedVertices`: the inverted boundary points
/// (mesh labels) of the surface, checking only boundary points `b` with
/// `active[b]` when `active` is given. `engine`'s geometry must be current
/// for `points`.
pub fn check_inverted_vertices(engine: &SurfaceEngine, part: &SurfacePartitioner, points: &[Vec3], active: Option<&[bool]>) -> BTreeSet<usize> {
    let is_active = |b: usize| active.is_none_or(|a| a[b]);
    // Per-patch normals at corners and edge points.
    let mut patch_normal: BTreeMap<usize, BTreeMap<usize, Vec3>> = BTreeMap::new();
    for &b in part.corners.iter().chain(part.edge_points.iter()) {
        if !is_active(b) {
            continue;
        }
        let pn = patch_normal.entry(b).or_default();
        for &bf in &engine.point_faces[b] {
            let e = pn.entry(part.face_patches[bf]).or_insert(Vec3::ZERO);
            *e = e.add(engine.face_normals[bf]);
        }
    }
    for pn in patch_normal.values_mut() {
        for n in pn.values_mut() {
            *n = n.scale(1.0 / (n.length() + VSMALL));
        }
    }
    let tri_normal = |a: Vec3, b: Vec3, c: Vec3| b.sub(a).cross(c.sub(a)).scale(0.5);
    let mut inverted = BTreeSet::new();
    for b in 0..engine.n_boundary_points() {
        if !is_active(b) {
            continue;
        }
        for (&bf, &pos) in engine.point_faces[b].iter().zip(&engine.point_in_faces[b]) {
            let p_normal = match patch_normal.get(&b) {
                Some(pn) => pn.get(&part.face_patches[bf]).copied().unwrap_or(Vec3::ZERO),
                None => engine.point_normals[b],
            };
            let ring = &engine.face_rings[bf];
            let k = ring.len();
            let (pa, pn_, pp) = (points[ring[pos]], points[ring[(pos + 1) % k]], points[ring[(pos + k - 1) % k]]);
            let fc = engine.face_centres[bf];
            // The triangle with the next point.
            let n_next = tri_normal(pa, pn_, fc);
            let m_next = n_next.length();
            if m_next < VSMALL || mag_sqr(pa.sub(pn_)) < VSMALL || mag_sqr(fc.sub(pa)) < VSMALL {
                inverted.insert(ring[pos]);
                break;
            }
            let n_next = n_next.scale(1.0 / m_next);
            if n_next.dot(p_normal) < 0.0 {
                inverted.insert(ring[pos]);
                break;
            }
            // The triangle with the previous point.
            let n_prev = tri_normal(pa, fc, pp);
            let m_prev = n_prev.length();
            if m_prev < VSMALL || mag_sqr(pa.sub(fc)) < VSMALL || mag_sqr(pp.sub(pa)) < VSMALL {
                inverted.insert(ring[pos]);
                break;
            }
            let n_prev = n_prev.scale(1.0 / m_prev);
            if n_prev.dot(p_normal) < 0.0 || n_next.dot(n_prev) < 0.0 {
                inverted.insert(ring[pos]);
                break;
            }
        }
    }
    // Concave faces.
    for ring in &engine.face_rings {
        let (ok, ok_points) = is_face_convex_and_ok(points, ring);
        if !ok {
            for (i, &v) in ring.iter().enumerate() {
                if is_active(engine.bp[v]) && !ok_points[i] {
                    inverted.insert(v);
                }
            }
        }
    }
    inverted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve::carve_box;
    use crate::shapes::box_surface;
    use crate::volume_mesh::VolumeMesh;
    use petir::mathf::RealMath;

    /// A closed hexagonal bipyramid: centre point 0 at `c` on the top fan
    /// (z = 0, six triangles, patch 0), ring points 1–6 at unit radius,
    /// apex 7 at (0, 0, −1) (six triangles, patch 1). One cell, every face
    /// on the boundary, wound outward.
    fn bipyramid(c: Vec3) -> VolumeMesh {
        let mut points = vec![c];
        for k in 0..6 {
            let a = k as f64 * std::f64::consts::PI / 3.0;
            points.push(Vec3::new(a.r_cos(), a.r_sin(), 0.0));
        }
        points.push(Vec3::new(0.0, 0.0, -1.0));
        let mut faces = Vec::new();
        for k in 0..6 {
            faces.push(vec![0, 1 + k, 1 + (k + 1) % 6]);
        }
        for k in 0..6 {
            faces.push(vec![7, 1 + (k + 1) % 6, 1 + k]);
        }
        let n = faces.len();
        VolumeMesh {
            points,
            faces,
            owner: vec![0; n],
            neighbour: vec![None; n],
            n_cells: 1,
            patches: vec![
                crate::volume_mesh::BoundaryPatch { name: "top".into(), start_face: 0, n_faces: 6 },
                crate::volume_mesh::BoundaryPatch { name: "bottom".into(), start_face: 6, n_faces: 6 },
            ],
        }
    }

    /// V&V (gh:#1258) — `meshSurfaceCheckInvertedVertices` by hand.
    /// Predicted (2026-11-05, before the first run): on the regular
    /// bipyramid nothing is inverted. With the top's centre dragged to
    /// (1.5, 0, 0), outside the hexagon past ring point 1 = (1, 0, 0), the
    /// two top triangles at point 1 (`[0, 6, 1]`, `[0, 1, 2]`) flip to −z.
    /// The centre (one patch: its normal is the unit sum of the top's area
    /// vectors, +z, since a fan's signed areas add to the hexagon's) is
    /// flagged; ring points 6 and 2 (edge points: their top-patch normal
    /// sums one flipped and one unflipped triangle, so one of the two
    /// disagrees with it) are flagged; ring point 1 is **not** (both its
    /// top triangles flipped: they agree with each other and with its
    /// top-patch normal, −z). The coverage plan's "flags exactly that
    /// point" is therefore predicted to miss: {0, 2, 6}.
    /// **Result (2026-11-05):** {0, 2, 6}, as predicted; with only the
    /// centre active, {0}.
    #[test]
    fn inverted_vertices_on_a_dragged_fan() {
        let m = bipyramid(Vec3::ZERO);
        let e = SurfaceEngine::new(&m);
        let part = SurfacePartitioner::new(&e);
        assert_eq!(part.edge_points.len(), 6);
        assert!(check_inverted_vertices(&e, &part, &m.points, None).is_empty());
        let m = bipyramid(Vec3::new(1.5, 0.0, 0.0));
        let e = SurfaceEngine::new(&m);
        let part = SurfacePartitioner::new(&e);
        let inv = check_inverted_vertices(&e, &part, &m.points, None);
        assert_eq!(inv.into_iter().collect::<Vec<_>>(), vec![0, 2, 6]);
        // Only the centre active: only the centre reported.
        let mut active = vec![false; e.n_boundary_points()];
        active[e.bp[0]] = true;
        assert_eq!(check_inverted_vertices(&e, &part, &m.points, Some(&active)).into_iter().collect::<Vec<_>>(), vec![0]);
    }

    /// V&V (gh:#1258) — `surfaceOptimizer::optimizePoint` on a planar fan.
    /// Predicted (2026-11-05, before the first run): (a) from the centre
    /// dragged to (1.5, 0) the optimised point lies inside the hexagon:
    /// all six triangle areas positive (the functional is infinite-like on
    /// a fold, `Σ L²/A` with `A` stabilised to ~1e-15·L²); (b) on the
    /// regular hexagon the centre is **not** a fixed point to 1e-12, as the
    /// coverage plan wrote: divide and conquer starts at the box centre
    /// and its first step moves to one of four quarter points, so the
    /// result depends on how close the Newton steps get before the
    /// relative-change test (`tol`) stops them. Predicted within 1e-3 of
    /// the radius at `tol` 0.001 (`newPositionSurfaceOptimizer`'s), and
    /// the functional within 1e-6 relative of its value at the centre.
    /// **Result (2026-11-05):** (a) held: from (1.5, 0) the point went to
    /// (1.33e-5, 1.16e-5), all six areas 0.43300–0.43302 > 0; (b) held:
    /// the regular hexagon's centre went to the **same** point (|d| 1.77e-5
    /// of a unit radius), functional 27.712812921 → 27.712812927 (2e-10
    /// relative). Same point from both starts because divide and conquer
    /// discards the starting position (it begins at the box centre): the
    /// optimiser's answer depends on the ring only.
    #[test]
    fn surface_optimizer_untangles_a_planar_fan() {
        let ring: Vec<Vec3> = (0..6).map(|k| {
            let a = k as f64 * std::f64::consts::PI / 3.0;
            Vec3::new(a.r_cos(), a.r_sin(), 0.0)
        }).collect();
        let trias: Vec<[usize; 3]> = (0..6).map(|k| [0, 1 + k, 1 + (k + 1) % 6]).collect();
        let mut pts = vec![Vec3::new(1.5, 0.0, 0.0)];
        pts.extend(ring.iter().copied());
        let k0 = stabilisation_factor(&pts, &trias);
        assert!(trias.iter().any(|t| area_xy(pts[t[0]], pts[t[1]], pts[t[2]]) < 0.0));
        let _ = k0;
        let p = optimize_point(&mut pts, &trias, 0.001);
        pts[0] = p;
        let areas: Vec<f64> = trias.iter().map(|t| area_xy(pts[t[0]], pts[t[1]], pts[t[2]])).collect();
        println!("dragged fan: optimised to {p:?}, areas {areas:?}");
        assert!(areas.iter().all(|&a| a > 0.0));

        let mut pts = vec![Vec3::ZERO];
        pts.extend(ring.iter().copied());
        let f_centre = evaluate_func(&pts, &trias, 0.0);
        let p = optimize_point(&mut pts, &trias, 0.001);
        pts[0] = p;
        let f_opt = evaluate_func(&pts, &trias, 0.0);
        println!("regular hexagon: centre went to {p:?} (|d| {:.3e}), func {f_centre} -> {f_opt}", p.length());
        assert!(p.length() < 1e-3);
        assert!((f_opt - f_centre).abs() / f_centre < 1e-6);
    }

    /// V&V (gh:#1258) — `partTriMesh` on the carved 4×4×4 box (96 quads,
    /// 98 points). Predicted (2026-11-05, before the first run): 98 + 96
    /// = 194 triangulation points, 96 × 4 × 2 = 768 triangles; the simplex
    /// of a point inside a side (four quads) has 4 × (2 + 1) = 12
    /// triangles (two centre triangles per face, one shape triangle) and
    /// its normal is the side's axis normal to 1e-12; moving that point
    /// with `update_vertex` moves each of its four face centres to the
    /// area-weighted centre of the face's four centre triangles.
    /// **Result (2026-11-05):** counts, simplex and normal held; the centre
    /// update first missed by 1 ulp because the test recomputed it from
    /// the already-updated centre (the centre is a corner of the triangles
    /// it averages over); against the pre-update centre it is exact. The
    /// carved box has no inverted vertex.
    #[test]
    fn part_tri_mesh_on_a_carved_box() {
        let (p, t) = box_surface(Vec3::ZERO, Vec3::new(2.0, 2.0, 2.0));
        let m = carve_box(&p, &t, 0.5);
        let e = SurfaceEngine::new(&m);
        let part = SurfacePartitioner::new(&e);
        let mut tm = PartTriMesh::new(&e, &part, &m.points);
        assert!(check_inverted_vertices(&e, &part, &m.points, None).is_empty());
        assert_eq!(tm.points.len(), 194);
        assert_eq!(tm.triangles.len(), 768);
        assert_eq!(tm.point_type.iter().filter(|&&t| t == FACECENTRE).count(), 96);
        // A point in the middle of the z = 2 side.
        let b = (0..e.n_boundary_points()).find(|&b| {
            let x = m.points[e.boundary_points[b]];
            (x.z - 2.0).abs() < 1e-12 && (x.x - 1.0).abs() < 1e-12 && (x.y - 1.0).abs() < 1e-12
        }).unwrap();
        let tp = tm.mesh_surface_point_label_in_tri_mesh[b];
        let s = PartTriMeshSimplex::new(&tm, tp).unwrap();
        assert_eq!(s.trias.len(), 12);
        let n = s.normal();
        assert!(n.x.abs() < 1e-12 && n.y.abs() < 1e-12 && (n.z - 1.0).abs() < 1e-12, "{n:?}");
        // The centre is itself a corner of the triangles it is averaged
        // over, so the expected value is taken with the old centre (first
        // run asserted it against the updated one and missed by 1 ulp).
        let mut before = tm.clone();
        before.points[tp] = Vec3::new(1.1, 1.05, 2.0);
        tm.update_vertex(tp, Vec3::new(1.1, 1.05, 2.0));
        let mut n_centres = 0;
        for &tri in &tm.point_triangles[tp] {
            let c = tm.triangles[tri].0[2];
            if tm.point_type[c] & FACECENTRE != 0 {
                let want = centre_of(&before.points, &before.triangles, &before.point_triangles[c]);
                assert_eq!(tm.points[c], want);
                assert!((tm.points[c].z - 2.0).abs() < 1e-12);
                n_centres += 1;
            }
        }
        assert_eq!(n_centres, 8, "two centre triangles per face, four faces");
        // around_points: the four faces at the point and one ring more.
        let inv: BTreeSet<usize> = [e.boundary_points[b]].into_iter().collect();
        let local = PartTriMesh::around_points(&e, &part, &m.points, &inv, 0);
        assert_eq!(local.triangles.len(), 4 * 8);
        let wider = PartTriMesh::around_points(&e, &part, &m.points, &inv, 1);
        assert_eq!(wider.triangles.len(), 16 * 8);
    }

    /// Differential (gh:#1258, plan item 4): `check_inverted_vertices`
    /// against `surface_mapper::cells_with_nonpositive_pyramids` on the
    /// 48-sided pipe (R 10 mm, L 60 mm) at h 6 mm, per-point dual with
    /// `Corners`, mapped onto the patches **without** the revert (the raw
    /// `map_vertices_onto_surface_patches`). They test different things
    /// (surface points against volume cells), so the overlap is recorded,
    /// not required equal. Predicted (2026-11-05, before the run): both
    /// non-empty (gh:#1241 reverted 64 points at h 6 mm on the L 200 mm
    /// pipe); at least 80 % of the bad cells that have a boundary face
    /// have an inverted vertex on it; inverted vertices outside every bad
    /// cell exist (a fold of the surface need not put a cell centre behind
    /// a face). The cell-centre dual (feature `cell-centre-dual`) is run
    /// too when built.
    ///
    /// **Result (2026-11-05;** per-point and cell-centre give the same
    /// boundary, so the same numbers): 1 390 boundary points; mapped
    /// without the revert, **4** cells with a non-positive pyramid, all 4
    /// with a boundary face carrying an inverted vertex (12 such vertices):
    /// the 80 % prediction held (100 %). But **554** vertices are inverted,
    /// and the same 554 before the mapping: 550 of them are reflex corners
    /// (`isFaceConvexAndOk`, `(e_prev × e_next)·n < −0.05`) of 156
    /// boundary faces (15 inlet, 15 outlet, 126 walls), the worst turning
    /// a right angle backwards. So by cfMesh's own definition this crate's
    /// dual boundary is "inverted" at 40 % of its points before anything
    /// is mapped: the dual's boundary polygons are not convex. Not
    /// predicted; it decides what `untangleSurface` (gh:#1259) will treat.
    #[test]
    fn inverted_vertices_against_nonpositive_pyramids_on_the_pipe() {
        use crate::dual::{DualKind, FeaturePointRule};
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.06, 48);
        let reflex = |e: &SurfaceEngine, pts: &[Vec3]| -> BTreeSet<usize> {
            e.face_rings
                .iter()
                .flat_map(|r| {
                    let (_, ok) = is_face_convex_and_ok(pts, r);
                    r.iter().zip(ok).filter(|(_, o)| !o).map(|(&v, _)| v).collect::<Vec<_>>()
                })
                .collect()
        };
        let mut kinds = vec![DualKind::MedianPerPoint];
        if cfg!(feature = "cell-centre-dual") {
            kinds.push(DualKind::CellCentre);
        }
        for kind in kinds {
            let o = crate::pipeline::TetDualOptions {
                cell_size: 0.006,
                n_layers: 0,
                feature_edges: false,
                dual_kind: kind,
                dual_feature_points: FeaturePointRule::Corners,
                ..Default::default()
            };
            let (m, _) = crate::pipeline::surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
            let e0 = SurfaceEngine::new(&m);
            let inv0 = check_inverted_vertices(&e0, &SurfacePartitioner::new(&e0), &m.points, None);
            let reflex0 = reflex(&e0, &m.points);
            let mapped = crate::surface_mapper::map_vertices_onto_surface_patches(&m, &p, &t, &regions);
            let bad = crate::surface_mapper::cells_with_nonpositive_pyramids(&mapped);
            let e = SurfaceEngine::new(&mapped);
            let part = SurfacePartitioner::new(&e);
            let inv = check_inverted_vertices(&e, &part, &mapped.points, None);
            let reflex1 = reflex(&e, &mapped.points);
            let (mut bad_bnd, mut bad_bnd_with_inv) = (0, 0);
            let mut inv_in_bad: BTreeSet<usize> = BTreeSet::new();
            for &c in &bad {
                let fs: Vec<usize> = (0..e.n_boundary_faces()).filter(|&bf| e.face_owners[bf] == c).collect();
                if fs.is_empty() {
                    continue;
                }
                bad_bnd += 1;
                let hit: Vec<usize> = fs.iter().flat_map(|&bf| e.face_rings[bf].iter().copied()).filter(|v| inv.contains(v)).collect();
                if !hit.is_empty() {
                    bad_bnd_with_inv += 1;
                }
                inv_in_bad.extend(hit);
            }
            println!(
                "pipe h 6 mm {kind:?} Corners, {} boundary points. Unmapped: {} inverted ({} reflex face corners). Mapped, no revert: {} inverted ({} reflex), {} cells with a non-positive pyramid ({} with a boundary face, {} of those with an inverted vertex), {} inverted vertices on such a face",
                e.n_boundary_points(),
                inv0.len(),
                reflex0.len(),
                inv.len(),
                reflex1.len(),
                bad.len(),
                bad_bnd,
                bad_bnd_with_inv,
                inv_in_bad.len()
            );
            // Where the reflex corners are: per patch, and the worst turn.
            let mut per_patch = vec![0usize; m.patches.len() + 1];
            let mut worst = 0.0f64;
            for (bf, r) in e0.face_rings.iter().enumerate() {
                let (ok, _) = is_face_convex_and_ok(&m.points, r);
                if !ok {
                    per_patch[e0.face_patches[bf]] += 1;
                    let n = ring_area_vector(&m.points, r).normalize();
                    let k = r.len();
                    for i in 0..k {
                        let a = m.points[r[i]].sub(m.points[r[(i + k - 1) % k]]).normalize();
                        let b = m.points[r[(i + 1) % k]].sub(m.points[r[i]]).normalize();
                        worst = worst.min(a.cross(b).dot(n));
                    }
                }
            }
            let names: Vec<&str> = m.patches.iter().map(|q| q.name.as_str()).collect();
            println!("  unmapped faces failing isFaceConvexAndOk per patch {names:?}: {per_patch:?}; most negative (e_prev x e_next).n = {worst:.3}");
            assert!(!inv.is_empty());
        }
    }
}
