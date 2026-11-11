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
use petir::mathf::RealMath;

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
        let c = points[tri[0]].add(points[tri[1]]).add(points[tri[2]]).div(3.0);
        let area = 0.5 * points[tri[1]].sub(points[tri[0]]).cross(points[tri[2]].sub(points[tri[0]])).length() + VSMALL;
        centre = centre.add(c.scale(area));
        area_sum += area;
    }
    centre.div(area_sum)
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
                hgas[i][j] = 0.5 * ((ga[i] * ga[j]) / stab - a * a * (ga[i] * ga[j]) / stab.r_powf(3.0));
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

/// Set by `new_position_surface_optimizer` while it runs the boundary point
/// named by `GH1385_BP`, so [`divide_and_conquer`] / [`steepest_descent`]
/// print what upstream's instrumented `surfaceOptimizer.C` prints (gh:#1385).
static SO_TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn so_trace_on() -> bool {
    SO_TRACE.load(std::sync::atomic::Ordering::Relaxed)
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
    for it in 0..100 {
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
        if so_trace_on() {
            eprintln!("DC {it} p {:.17e} {:.17e} f {:.17e}", pts[c].x, pts[c].y, func_after);
        }
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
    for it in 0..100u8 {
        let func_before = func_after;
        let (g, h) = evaluate_gradients(pts, trias, k);
        let det = h[0][0] * h[1][1] - h[0][1] * h[1][0];
        let mut disp = Vec3::ZERO;
        if det.abs() >= VSMALL {
            disp.x = (h[1][1] * g[0] - h[0][1] * g[1]) / det;
            disp.y = (-h[1][0] * g[0] + h[0][0] * g[1]) / det;
            let m = disp.length();
            if m > 0.2 * avg_edge {
                // `dir = disp/mag(disp); disp = dir*0.2*avgEdge`
                let dir = disp.div(m);
                disp = dir.scale(0.2).scale(avg_edge);
            }
        }
        pts[c] = pts[c].sub(disp);
        k = stabilisation_factor(pts, trias);
        func_after = evaluate_func(pts, trias, k);
        if so_trace_on() {
            eprintln!("SD {it} g {:.17e} {:.17e} H {:.17e} {:.17e} {:.17e} {:.17e} disp {:.17e} {:.17e} p {:.17e} {:.17e} f {:.17e}", g[0], g[1], h[0][0], h[0][1], h[1][0], h[1][1], disp.x, disp.y, pts[c].x, pts[c].y, func_after);
        }
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
    // `pts_[i] /= scale`, `pMin_ /= scale`: each component divided
    for p in pts.iter_mut() {
        *p = p.div(scale);
    }
    let (p_min, p_max) = (p_min.div(scale), p_max.div(scale));
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

// ---------------------------------------------------------------------------
// meshSurfaceOptimizer (gh:#1259)
// ---------------------------------------------------------------------------

/// `meshSurfaceOptimizer::vertexTypes`: a point inside a patch.
pub const PARTITION: u8 = 1;
/// A feature-edge point.
pub const EDGE: u8 = 2;
/// A corner (never moved).
pub const VERTEX_CORNER: u8 = 4;
/// A point the caller locked.
pub const LOCKED: u8 = 16;

/// What [`MeshSurfaceOptimizer::untangle_surface`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UntangleReport {
    /// Whether any point was smoothed.
    pub changed: bool,
    /// Inverted points at the first check.
    pub n_inverted_first: usize,
    /// Inverted points at the last check (0: untangled).
    pub n_inverted_last: usize,
    /// Global iterations run (upstream stops at 10).
    pub global_iterations: usize,
    /// Smoothing passes run, over all global iterations.
    pub passes: usize,
}

/// cfMesh `meshSurfaceOptimizer` (serial, 3-D): untangles and smooths the
/// boundary points of a mesh within the surface, optionally mapping them
/// back onto the input surface (upstream's octree constructors; here
/// [`crate::surface_mapper::SurfaceQueries`]).
///
/// The partitioner's patches must be the surface's region indices when a
/// mapper is given (`mapEdgeNodes` asks for the nearest point of each
/// patch). Not ported: ~~`optimizeSurface2D`, `untangleSurface2D`,~~
/// (**ported 2026-11-05, gh:#1275 remainder**: [`optimize_surface_2d`],
/// [`untangle_surface_2d`]) `enforceConstraints` (upstream throws and writes a point subset; there
/// is no subset here), and the parallel exchanges.
#[derive(Debug, Clone)]
pub struct MeshSurfaceOptimizer {
    /// The surface (geometry kept current with the points).
    pub engine: SurfaceEngine,
    /// Its partition into patches.
    pub part: SurfacePartitioner,
    vertex_type: Vec<u8>,
    tri_mesh: Option<PartTriMesh>,
    mapper: Option<crate::surface_mapper::SurfaceQueries>,
}

/// `plane::nearestPoint` for the plane through `base` with unit normal `n`.
fn plane_nearest(base: Vec3, n: Vec3, p: Vec3) -> Vec3 {
    p.sub(n.scale(p.sub(base).dot(n)))
}

impl MeshSurfaceOptimizer {
    /// The optimiser on `engine` partitioned by `part`, mapping onto
    /// `mapper` when given. Corners are `CORNER`, edge points `EDGE`, the
    /// rest `PARTITION` (`classifySurfaceVertices`).
    pub fn new(engine: SurfaceEngine, part: SurfacePartitioner, mapper: Option<crate::surface_mapper::SurfaceQueries>) -> Self {
        let mut vertex_type = vec![PARTITION; engine.n_boundary_points()];
        for &b in &part.corners {
            vertex_type[b] = VERTEX_CORNER;
        }
        for &b in &part.edge_points {
            vertex_type[b] = EDGE;
        }
        MeshSurfaceOptimizer { engine, part, vertex_type, tri_mesh: None, mapper }
    }

    /// `lockBoundaryFaces`: the points of these boundary faces never move.
    pub fn lock_boundary_faces(&mut self, faces: &[usize]) {
        for &bf in faces {
            for &v in &self.engine.face_rings[bf] {
                self.vertex_type[self.engine.bp[v]] |= LOCKED;
            }
        }
    }

    /// `lockBoundaryPoints` (boundary labels).
    pub fn lock_boundary_points(&mut self, points: &[usize]) {
        for &b in points {
            self.vertex_type[b] |= LOCKED;
        }
    }

    /// `lockFeatureEdges`: lock every edge point and corner.
    pub fn lock_feature_edges(&mut self) {
        for t in self.vertex_type.iter_mut() {
            if *t & (EDGE | VERTEX_CORNER) != 0 {
                *t |= LOCKED;
            }
        }
    }

    /// `removeUserConstraints`.
    pub fn remove_user_constraints(&mut self) {
        for t in self.vertex_type.iter_mut() {
            *t &= !LOCKED;
        }
    }

    fn locked(&self, b: usize) -> bool {
        self.vertex_type[b] & LOCKED != 0
    }

    /// `findInvertedVertices`: mark the inverted points among the active
    /// ones (`smooth`), then `n_layers` rings of `pointPoints` around
    /// them; the number inverted.
    fn find_inverted_vertices(&self, points: &[Vec3], smooth: &mut [bool], n_layers: usize) -> usize {
        let inverted = check_inverted_vertices(&self.engine, &self.part, points, Some(smooth));
        let mut n = 0;
        for (b, s) in smooth.iter_mut().enumerate() {
            *s = inverted.contains(&self.engine.boundary_points[b]);
            n += usize::from(*s);
        }
        if n == 0 {
            return 0;
        }
        for _ in 0..n_layers {
            let orig = smooth.to_vec();
            for (b, &o) in orig.iter().enumerate() {
                if o {
                    for &q in &self.engine.point_points[b] {
                        smooth[q] = true;
                    }
                }
            }
        }
        n
    }

    /// `newEdgePositionLaplacian`: the average of the two feature-edge
    /// neighbours (unchanged unless there are exactly two).
    fn new_edge_position_laplacian(&self, points: &[Vec3], b: usize) -> Vec3 {
        let v = self.engine.boundary_points[b];
        if self.locked(b) {
            return points[v];
        }
        let others: Vec<usize> = self.engine.point_edges[b]
            .iter()
            .filter(|e| self.part.feature_edges.contains(e))
            .map(|&e| {
                let [a, c] = self.engine.edges[e];
                if a == v {
                    c
                } else {
                    a
                }
            })
            .collect();
        if others.len() != 2 {
            return points[v];
        }
        points[others[0]].add(points[others[1]]).scale(0.5)
    }

    /// `newPositionLaplacianFC`: the average of the point's face centres,
    /// projected onto its tangent plane when `transform`.
    fn new_position_laplacian_fc(&self, points: &[Vec3], b: usize, transform: bool) -> Vec3 {
        let v = self.engine.boundary_points[b];
        if self.locked(b) {
            return points[v];
        }
        let pf = &self.engine.point_faces[b];
        let mut sum = Vec3::ZERO;
        if transform {
            let n = self.engine.point_normals[b];
            if mag_sqr(n) < VSMALL {
                return points[v];
            }
            let n = n.div(n.length());
            for &bf in pf {
                sum = sum.add(plane_nearest(points[v], n, self.engine.face_centres[bf]));
            }
        } else {
            for &bf in pf {
                sum = sum.add(self.engine.face_centres[bf]);
            }
        }
        sum.div(pf.len() as f64)
    }

    /// `transformIntoPlane`: the point's simplex in the tangent plane
    /// (x along the first simplex point that does not project onto the
    /// centre).
    fn transform_into_plane(&self, b: usize, base: Vec3, n: Vec3) -> Option<(Vec3, Vec3, Vec<Vec3>, Vec<[usize; 3]>)> {
        let tm = self.tri_mesh.as_ref()?;
        let tp = tm.mesh_surface_point_label_in_tri_mesh[b];
        if tp == NO_POINT {
            return None;
        }
        let simplex = PartTriMeshSimplex::new(tm, tp)?;
        let p = simplex.centre_point();
        let mut axes = None;
        for &sp in &simplex.pts {
            let q = plane_nearest(base, n, sp);
            let d = q.sub(p).length();
            if d > VSMALL {
                let x = q.sub(p).div(d);
                let y = n.cross(x);
                axes = Some((x, y.div(y.length())));
                break;
            }
        }
        let (x, y) = axes?;
        let pts = simplex.pts.iter().map(|&sp| Vec3::new(sp.sub(p).dot(x), sp.sub(p).dot(y), 0.0)).collect();
        Some((x, y, pts, simplex.trias))
    }

    /// `newPositionSurfaceOptimizer` (`tol` 0.001).
    fn new_position_surface_optimizer(&self, points: &[Vec3], b: usize) -> Vec3 {
        let v = self.engine.boundary_points[b];
        if self.locked(b) {
            return points[v];
        }
        let n = self.engine.point_normals[b];
        if mag_sqr(n) < VSMALL {
            return points[v];
        }
        let n = n.div(n.length());
        let Some((x, y, mut pts, trias)) = self.transform_into_plane(b, points[v], n) else {
            return points[v];
        };
        let pts_in = pts.clone();
        let tracing = std::env::var("GH1385_TRACE").is_ok() && std::env::var("GH1385_BP").ok().and_then(|v| v.parse::<usize>().ok()) == Some(b);
        SO_TRACE.store(tracing, std::sync::atomic::Ordering::Relaxed);
        let q = optimize_point(&mut pts, &trias, 0.001);
        SO_TRACE.store(false, std::sync::atomic::Ordering::Relaxed);
        if std::env::var("GH1385_TRACE").is_ok() && std::env::var("GH1385_BP").ok().and_then(|v| v.parse::<usize>().ok()) == Some(b) {
            // gh:#1385: the instrumented upstream prints the same (`SO ...`)
            eprintln!("SO pn {:.17e} {:.17e} {:.17e}", self.engine.point_normals[b].x, self.engine.point_normals[b].y, self.engine.point_normals[b].z);
            eprintln!("SO vx {:.17e} {:.17e} {:.17e}", x.x, x.y, x.z);
            eprintln!("SO vy {:.17e} {:.17e} {:.17e}", y.x, y.y, y.z);
            eprintln!("SO new {:.17e} {:.17e} {:.17e}", q.x, q.y, q.z);
            for (i, p) in pts_in.iter().enumerate() {
                eprintln!("SO pt {i} {:.17e} {:.17e}", p.x, p.y);
            }
        }
        let new_p = points[v].add(x.scale(q.x)).add(y.scale(q.y));
        if new_p.x.is_finite() && new_p.y.is_finite() && new_p.z.is_finite() {
            new_p
        } else {
            points[v]
        }
    }

    /// `smoothEdgePoints`.
    fn smooth_edge_points(&mut self, points: &mut [Vec3], edge_points: &[usize]) {
        let new: Vec<(usize, Vec3)> =
            edge_points.iter().filter(|&&b| !self.locked(b)).map(|&b| (b, self.new_edge_position_laplacian(points, b))).collect();
        for (b, p) in new {
            self.engine.move_boundary_vertex_no_update(points, b, p);
        }
        self.engine.update_geometry(points, edge_points);
    }

    /// `smoothLaplacianFC`.
    pub fn smooth_laplacian_fc(&mut self, points: &mut [Vec3], selected: &[usize], transform: bool) {
        let new: Vec<(usize, Vec3)> =
            selected.iter().filter(|&&b| !self.locked(b)).map(|&b| (b, self.new_position_laplacian_fc(points, b, transform))).collect();
        for (b, p) in new {
            self.engine.move_boundary_vertex_no_update(points, b, p);
        }
        self.engine.update_geometry(points, selected);
    }

    /// `smoothSurfaceOptimizer`. The triangulation is built on first use
    /// and afterwards updated only at `selected`, as upstream's.
    pub fn smooth_surface_optimizer(&mut self, points: &mut [Vec3], selected: &[usize]) {
        if self.tri_mesh.is_none() {
            self.tri_mesh = Some(PartTriMesh::new(&self.engine, &self.part, points));
        }
        if let Some(tm) = self.tri_mesh.as_mut() {
            tm.update_vertices(&self.engine, points, selected);
        }
        let new: Vec<Vec3> = selected.iter().map(|&b| self.new_position_surface_optimizer(points, b)).collect();
        for (&b, p) in selected.iter().zip(new) {
            self.engine.move_boundary_vertex_no_update(points, b, p);
        }
        self.engine.update_geometry(points, selected);
    }

    /// `findMappingDistance` with upstream's face centres.
    fn mapping_distance(&self, points: &[Vec3], b: usize) -> f64 {
        let p = points[self.engine.boundary_points[b]];
        4.0 * self.engine.point_faces[b].iter().map(|&bf| mag_sqr(self.engine.face_centres[bf].sub(p))).fold(0.0, f64::max)
    }

    /// `meshSurfaceMapper::mapEdgeNodes(nodes)`.
    fn map_edge_nodes(&mut self, points: &mut [Vec3], nodes: &[usize]) {
        let Some(q) = self.mapper.as_ref() else { return };
        let dist: Vec<f64> = nodes.iter().map(|&b| self.mapping_distance(points, b)).collect();
        if let Some(o) = q.octree() {
            // gh:#1385: upstream's routine, patches unsorted
            let moved: Vec<(usize, Vec3)> = nodes
                .iter()
                .zip(&dist)
                .map(|(&b, &max_dist)| {
                    let v = self.engine.boundary_points[b];
                    (v, crate::surface_mapper::map_edge_node_upstream(q, o, points[v], &self.part.point_patches[b], max_dist))
                })
                .collect();
            for (v, p) in moved {
                points[v] = p;
            }
            self.engine.update_geometry(points, nodes);
            return;
        }
        for (&b, &max2) in nodes.iter().zip(&dist) {
            let mut ps = self.part.point_patches[b].clone();
            ps.sort_unstable();
            let v = self.engine.boundary_points[b];
            points[v] = crate::surface_mapper::map_edge_node(q, points[v], &ps, max2);
        }
        self.engine.update_geometry(points, nodes);
    }

    /// `meshSurfaceMapper::mapVerticesOntoSurface(nodes)`: each to the
    /// nearest point of the surface, whatever its patch.
    fn map_vertices_onto_surface(&mut self, points: &mut [Vec3], nodes: &[usize]) {
        let Some(q) = self.mapper.as_ref() else { return };
        for &b in nodes {
            let v = self.engine.boundary_points[b];
            points[v] = q.nearest(points[v]);
        }
        self.engine.update_geometry(points, nodes);
    }

    /// `untangleSurface(selectedBoundaryPoints, nAdditionalLayers)`.
    pub fn untangle_surface_selected(&mut self, points: &mut [Vec3], selected: &[usize], n_additional_layers: usize) -> UntangleReport {
        let nbp = self.engine.n_boundary_points();
        let mut rep = UntangleReport::default();
        let mut smooth = vec![false; nbp];
        for &b in selected {
            if !self.locked(b) {
                smooth[b] = true;
            }
        }
        let mut remap = true;
        let mut n_inverted;
        let mut min_inverted = nbp;
        let mut history: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        let mut min_points: Vec<Vec3> = vec![Vec3::ZERO; nbp];
        let mut first = true;
        loop {
            let (mut n_iter, mut n_after_refresh) = (0usize, 0usize);
            loop {
                n_inverted = self.find_inverted_vertices(points, &mut smooth, n_additional_layers);
                if first {
                    rep.n_inverted_first = n_inverted;
                    first = false;
                }
                if n_inverted == 0 {
                    break;
                }
                if n_inverted < min_inverted {
                    min_inverted = n_inverted;
                    n_after_refresh = 0;
                    for (b, mp) in min_points.iter_mut().enumerate() {
                        *mp = points[self.engine.boundary_points[b]];
                    }
                }
                n_after_refresh += 1;
                history.push_back(n_inverted);
                if history.len() > 2 {
                    history.pop_front();
                }
                if !history.contains(&min_inverted) || n_after_refresh > 2 {
                    break;
                }
                rep.changed = true;
                rep.passes += 1;
                let (mut moved, mut moved_edge) = (Vec::new(), Vec::new());
                for (b, &s) in smooth.iter().enumerate() {
                    if !s {
                        continue;
                    }
                    if self.vertex_type[b] & PARTITION != 0 {
                        moved.push(b);
                    } else if self.vertex_type[b] & EDGE != 0 {
                        moved_edge.push(b);
                    }
                }
                self.smooth_edge_points(points, &moved_edge);
                if remap {
                    self.map_edge_nodes(points, &moved_edge);
                }
                self.engine.update_geometry(points, &moved_edge);
                self.smooth_laplacian_fc(points, &moved, true);
                self.engine.update_geometry(points, &moved);
                self.smooth_surface_optimizer(points, &moved);
                if remap {
                    self.map_vertices_onto_surface(points, &moved);
                }
                self.engine.update_geometry(points, &moved);
                n_iter += 1;
                if !(n_inverted > 0 && n_iter < 20) {
                    break;
                }
            }
            if n_inverted > 0 {
                for (b, &mp) in min_points.iter().enumerate() {
                    self.engine.move_boundary_vertex_no_update(points, b, mp);
                }
                self.engine.update_geometry_all(points);
                let moved: Vec<usize> = (0..nbp).filter(|&b| smooth[b]).collect();
                self.smooth_laplacian_fc(points, &moved, false);
                if remap {
                    self.map_vertices_onto_surface(points, &moved);
                }
                self.engine.update_geometry(points, &moved);
                if rep.global_iterations > 5 {
                    remap = false;
                }
            }
            rep.global_iterations += 1;
            if !(n_inverted > 0 && rep.global_iterations < 10) {
                break;
            }
        }
        rep.n_inverted_last = n_inverted;
        rep
    }

    /// `untangleSurface(nAdditionalLayers)`: every boundary point (upstream's
    /// default is 2 layers).
    pub fn untangle_surface(&mut self, points: &mut [Vec3], n_additional_layers: usize) -> UntangleReport {
        let all: Vec<usize> = (0..self.engine.n_boundary_points()).collect();
        self.untangle_surface_selected(points, &all, n_additional_layers)
    }

    /// With `GH1385_TRACE` and `GH1385_BP=<boundary point>` set, print that
    /// boundary point as upstream's instrumented `meshSurfaceOptimizer` dumps
    /// it (`TRACE <tag> <iteration> <bp> <x> <y> <z>`, gh:#1385).
    fn trace_bp(&self, tag: &str, iter: usize, points: &[Vec3]) {
        if std::env::var("GH1385_TRACE").is_err() {
            return;
        }
        if let Some(bp) = std::env::var("GH1385_BP").ok().and_then(|v| v.parse::<usize>().ok()) {
            let p = points[self.engine.boundary_points[bp]];
            eprintln!("TRACE {tag} {iter} {bp} {:.17e} {:.17e} {:.17e}", p.x, p.y, p.z);
        }
    }

    /// `optimizeSurface(nIterations)`: `n_iterations` passes of edge-point
    /// smoothing (mapped back onto the feature edges when there is a
    /// mapper), then of Laplace (face centres, in plane) and
    /// `surfaceOptimizer` smoothing of the patch points, then
    /// `untangleSurface(0)`.
    pub fn optimize_surface(&mut self, points: &mut [Vec3], n_iterations: usize) -> UntangleReport {
        let (mut edge, mut partition) = (Vec::new(), Vec::new());
        for b in 0..self.engine.n_boundary_points() {
            if self.locked(b) {
                continue;
            }
            if self.vertex_type[b] & EDGE != 0 {
                edge.push(b);
            } else if self.vertex_type[b] & PARTITION != 0 {
                partition.push(b);
            }
        }
        for _ in 0..n_iterations {
            self.smooth_edge_points(points, &edge);
            self.map_edge_nodes(points, &edge);
            self.engine.update_geometry(points, &edge);
        }
        for i in 0..n_iterations {
            self.smooth_laplacian_fc(points, &partition, true);
            self.trace_bp("opt_lapfc", i, points);
            self.smooth_surface_optimizer(points, &partition);
            self.trace_bp("opt_so", i, points);
        }
        self.untangle_surface(points, 0)
    }
}

/// `tetMeshGenerator::mapMeshToSurface`: every boundary point of `mesh` to
/// the nearest point of the surface (`mapVerticesOntoSurface`), then
/// `meshSurfaceOptimizer(mse, octree).untangleSurface()` (2 additional
/// layers, mapping back onto the surface). The whole boundary is one patch
/// here, as upstream's at that stage (before `extractPatches`).
pub fn map_mesh_to_surface(mesh: &crate::volume_mesh::VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions) -> (crate::volume_mesh::VolumeMesh, UntangleReport) {
    let q = crate::surface_mapper::SurfaceQueries::new(points, tris, regions);
    map_mesh_to_surface_with(mesh, q)
}

/// [`map_mesh_to_surface`] with the nearest-point searches of upstream's
/// octree (`meshOctree::findNearestSurfacePoint`), as the generators call it
/// (gh:#1385).
pub fn map_mesh_to_surface_octree(mesh: &crate::volume_mesh::VolumeMesh, o: &crate::mesh_octree::MeshOctree, points: &[Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions) -> (crate::volume_mesh::VolumeMesh, UntangleReport) {
    let q = crate::surface_mapper::SurfaceQueries::new(points, tris, regions).with_octree(std::sync::Arc::new(o.clone()));
    map_mesh_to_surface_with(mesh, q)
}

fn map_mesh_to_surface_with(mesh: &crate::volume_mesh::VolumeMesh, q: crate::surface_mapper::SurfaceQueries) -> (crate::volume_mesh::VolumeMesh, UntangleReport) {
    let mut m = mesh.clone();
    let one = vec![0usize; m.face_count()];
    let mut engine = SurfaceEngine::from_parts(&m.points, &m.faces, &m.owner, &m.neighbour, &one);
    for b in 0..engine.n_boundary_points() {
        let v = engine.boundary_points[b];
        m.points[v] = q.nearest(m.points[v]);
    }
    engine.update_geometry_all(&m.points);
    let part = SurfacePartitioner::new(&engine);
    let mut opt = MeshSurfaceOptimizer::new(engine, part, Some(q));
    let rep = opt.untangle_surface(&mut m.points, 2);
    (m, rep)
}

/// `tetMeshGenerator::optimiseMeshSurface`:
/// `meshSurfaceOptimizer(mse, octree).optimizeSurface()` (5 iterations) on
/// `mesh`, whose boundary face `f` is in surface region `face_region[f]`.
pub fn optimise_mesh_surface(mesh: &mut crate::volume_mesh::VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions, face_region: &[Option<usize>]) -> UntangleReport {
    let q = crate::surface_mapper::SurfaceQueries::new(points, tris, regions);
    optimise_mesh_surface_with(mesh, q, regions, face_region)
}

/// [`optimise_mesh_surface`] with upstream's octree for the nearest-point
/// searches of `mapEdgeNodes` (gh:#1385), as the generators call it.
pub fn optimise_mesh_surface_octree(mesh: &mut crate::volume_mesh::VolumeMesh, o: &crate::mesh_octree::MeshOctree, points: &[Vec3], tris: &[[usize; 3]], regions: &crate::patches::SurfaceRegions, face_region: &[Option<usize>]) -> UntangleReport {
    let q = crate::surface_mapper::SurfaceQueries::new(points, tris, regions).with_octree(std::sync::Arc::new(o.clone()));
    optimise_mesh_surface_with(mesh, q, regions, face_region)
}

fn optimise_mesh_surface_with(mesh: &mut crate::volume_mesh::VolumeMesh, q: crate::surface_mapper::SurfaceQueries, regions: &crate::patches::SurfaceRegions, face_region: &[Option<usize>]) -> UntangleReport {
    let unassigned = regions.names.len();
    let patch_of: Vec<usize> = face_region.iter().map(|r| r.unwrap_or(unassigned)).collect();
    let engine = SurfaceEngine::from_parts(&mesh.points, &mesh.faces, &mesh.owner, &mesh.neighbour, &patch_of);
    let part = SurfacePartitioner::new(&engine);
    let mut opt = MeshSurfaceOptimizer::new(engine, part, Some(q));
    opt.optimize_surface(&mut mesh.points, 5)
}

/// What [`optimize_surface_2d`] and [`untangle_surface_2d`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Surface2DReport {
    /// `optimizeSurface2D`: outline points smoothed along the outline
    /// (`EDGE` z-min points with an active edge).
    pub n_edge_points: usize,
    /// `optimizeSurface2D`: z-min points inside the bottom patch smoothed.
    pub n_partition_points: usize,
    /// `untangleSurface2D`: bad faces (`findBadFaces`, among the changed
    /// faces) at each iteration, the last 0 when it untangled.
    pub bad_faces: Vec<usize>,
}

/// cfMesh `meshSurfaceOptimizer::optimizeSurface2D(nIterations)` (gh:#1275
/// remainder) on a mesh one cell thick whose side patch `i` is the surface
/// region `i`: `nIterations` times, the z-min outline points (`EDGE`, with
/// an edge across the thickness) by `smoothEdgePoints`, the z-max points
/// copied across (`polyMeshGen2DEngine::correctPoints`), and the active
/// edges mapped onto their patches (`meshSurfaceMapper2D::
/// mapVerticesOntoSurfacePatches(activeEdges)`); then `nIterations` times
/// the z-min points inside the bottom patch (`PARTITION`) by
/// `smoothSurfaceOptimizer`, and `correctPoints`. Upstream's default is 5
/// iterations.
pub fn optimize_surface_2d(mesh: &mut crate::volume_mesh::VolumeMesh, o: &crate::mesh_octree::MeshOctree, n_iterations: usize) -> Result<Surface2DReport, String> {
    let e2d = crate::poly_mesh_gen_2d_engine::PolyMeshGen2DEngine::new(mesh)?;
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let mut opt = MeshSurfaceOptimizer::new(engine, part, None);
    let (mut moved, mut active, mut update) = (Vec::new(), Vec::new(), Vec::new());
    for (e, &[s, t]) in opt.engine.edges.iter().enumerate() {
        if e2d.z_min_point[s] ^ e2d.z_min_point[t] {
            let b = if e2d.z_min_point[s] { opt.engine.bp[s] } else { opt.engine.bp[t] };
            if opt.vertex_type[b] & EDGE != 0 {
                active.push(e);
                update.push(opt.engine.bp[s]);
                update.push(opt.engine.bp[t]);
                moved.push(b);
            }
        }
    }
    for _ in 0..n_iterations {
        opt.smooth_edge_points(&mut mesh.points, &moved);
        e2d.correct_points(&mut mesh.points);
        let (engine, part) = (opt.engine.clone(), opt.part.clone());
        crate::surface_mapper_2d::map_edges_onto_surface_patches_2d(mesh, o, &engine, &part, &active)?;
        opt.engine.update_geometry(&mesh.points, &update);
    }
    let partition: Vec<usize> = (0..opt.engine.n_boundary_points()).filter(|&b| e2d.z_min_point[opt.engine.boundary_points[b]] && opt.vertex_type[b] & PARTITION != 0).collect();
    for _ in 0..n_iterations {
        opt.smooth_surface_optimizer(&mut mesh.points, &partition);
        e2d.correct_points(&mut mesh.points);
        opt.engine.update_geometry_all(&mesh.points);
    }
    Ok(Surface2DReport { n_edge_points: moved.len(), n_partition_points: partition.len(), bad_faces: Vec::new() })
}

/// cfMesh `meshSurfaceOptimizer::untangleSurface2D` (gh:#1275 remainder):
/// up to 20 times, the bad faces (`findBadFaces`, not relaxed, among the
/// faces changed by the last pass) activate their z-min points; those are
/// smoothed 5 times (`smoothEdgePoints` on the `EDGE` ones,
/// `smoothSurfaceOptimizer` on the `PARTITION` ones), then
/// `correctPoints`. Stops when no changed face is bad.
pub fn untangle_surface_2d(mesh: &mut crate::volume_mesh::VolumeMesh) -> Result<Surface2DReport, String> {
    let e2d = crate::poly_mesh_gen_2d_engine::PolyMeshGen2DEngine::new(mesh)?;
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let mut opt = MeshSurfaceOptimizer::new(engine, part, None);
    let mut point_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh.points.len()];
    for (f, ring) in mesh.faces.iter().enumerate() {
        for &v in ring {
            point_faces[v].push(f);
        }
    }
    let mut changed = vec![true; mesh.face_count()];
    let mut rep = Surface2DReport::default();
    for _ in 0..20 {
        let bad = crate::mesh_optimizer::find_bad_faces(mesh);
        let bad_faces: Vec<usize> = (0..mesh.face_count()).filter(|&f| bad[f] && changed[f]).collect();
        rep.bad_faces.push(bad_faces.len());
        if bad_faces.is_empty() {
            break;
        }
        let mut active = vec![false; opt.engine.n_boundary_points()];
        changed = vec![false; mesh.face_count()];
        for &f in &bad_faces {
            for &v in &mesh.faces[f] {
                if e2d.z_min_point[v] {
                    if let Some(&b) = opt.engine.bp.get(v).filter(|&&b| b != crate::surface_engine::NOT_BOUNDARY) {
                        active[b] = true;
                    }
                    for &g in &point_faces[v] {
                        changed[g] = true;
                    }
                }
            }
        }
        let (mut edge, mut moved) = (Vec::new(), Vec::new());
        for (b, &a) in active.iter().enumerate() {
            if !a {
                continue;
            }
            if opt.vertex_type[b] & EDGE != 0 {
                edge.push(b);
            } else if opt.vertex_type[b] & PARTITION != 0 {
                moved.push(b);
            }
        }
        for _ in 0..5 {
            opt.smooth_edge_points(&mut mesh.points, &edge);
            opt.smooth_surface_optimizer(&mut mesh.points, &moved);
        }
        e2d.correct_points(&mut mesh.points);
        opt.engine.update_geometry_all(&mesh.points);
    }
    Ok(rep)
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
            cells: None,
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

#[cfg(test)]
mod diag {
    use super::*;
    /// Diagnostic, `--ignored`: print the first non-convex boundary faces
    /// of the per-point `Corners` dual of the pipe (h 6 mm).
    #[test]
    #[ignore]
    fn diagnose_reflex_dual_faces() {
        use crate::dual::{DualKind, FeaturePointRule};
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.06, 48);
        let o = crate::pipeline::TetDualOptions { cell_size: 0.006, n_layers: 0, feature_edges: false, dual_kind: DualKind::MedianPerPoint, dual_feature_points: FeaturePointRule::Corners, ..Default::default() };
        let (m, _) = crate::pipeline::surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        let e = SurfaceEngine::new(&m);
        let mut shown = 0;
        for (bf, r) in e.face_rings.iter().enumerate() {
            let (ok, oks) = is_face_convex_and_ok(&m.points, r);
            if ok || shown >= 4 {
                continue;
            }
            shown += 1;
            println!("face {bf} patch {} ring {} pts ok {oks:?}", e.face_patches[bf], r.len());
            for &v in r {
                let x = m.points[v];
                println!("   {v}: ({:.5}, {:.5}, {:.5}) r {:.5}", x.x * 1e3, x.y * 1e3, x.z * 1e3, (x.x * x.x + x.y * x.y).sqrt() * 1e3);
            }
        }
    }
}

#[cfg(test)]
mod tests_gh1259 {
    use super::*;
    use crate::dual::{DualKind, FeaturePointRule};
    use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
    use crate::volume_mesh::VolumeMesh;
    use petir::mathf::RealMath;

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
            cells: None,
        }
    }

    /// V&V (gh:#1259) — `untangleSurface` by hand, no mapper, feature
    /// edges locked (so only the top's centre may move; the apex is a
    /// patch point of the bottom and is not inverted). Predicted
    /// (2026-11-05, before the first run): 3 inverted at the first check
    /// ({0, 2, 6}, the gh:#1258 test), 0 at the last; the centre ends
    /// inside the hexagon in its plane (z = 0 to 1e-12), every top triangle
    /// positive about +z; one smoothing pass; the ring and apex unmoved.
    /// **Result (2026-11-05):** 1 inverted at the first check, not 3 —
    /// **missed**: `untangleSurface` checks only the selected points, and
    /// a locked point is never selected, so ring points 2 and 6 (locked
    /// feature-edge points) are not checked at all; the rest held (0 at
    /// the last check, one pass, centre to (1.33e-5, 1.16e-5, 0), the
    /// point gh:#1258's `surfaceOptimizer` test found, ring
    /// unmoved). The apex did move — **missed**: it is a free bottom point
    /// two rings from the centre, which the two extra layers select; it
    /// moved 1.8e-5 in its tangent plane (the bottom's point normal, −z).
    #[test]
    fn untangle_surface_repairs_a_dragged_fan() {
        let mut m = bipyramid(Vec3::new(1.5, 0.0, 0.0));
        let orig = m.points.clone();
        let e = SurfaceEngine::new(&m);
        let part = SurfacePartitioner::new(&e);
        let mut so = MeshSurfaceOptimizer::new(e, part, None);
        // A user lock removed again leaves nothing locked.
        so.lock_boundary_points(&[0]);
        assert!(so.locked(0));
        so.remove_user_constraints();
        assert!(!so.locked(0));
        so.lock_feature_edges();
        let r = so.untangle_surface(&mut m.points, 2);
        println!("{r:?}; centre {:?}", m.points[0]);
        assert_eq!(r.n_inverted_first, 1);
        assert_eq!(r.n_inverted_last, 0);
        assert_eq!(r.passes, 1);
        assert!(m.points[0].z.abs() < 1e-12);
        for k in 0..6 {
            let (a, b) = (m.points[1 + k], m.points[1 + (k + 1) % 6]);
            assert!(a.sub(m.points[0]).cross(b.sub(m.points[0])).z > 0.0, "top triangle {k}");
        }
        for v in 1..7 {
            assert_eq!(m.points[v], orig[v]);
        }
        // The apex is two rings from the centre, so `untangleSurface`'s two
        // extra layers select it and the Laplace pass moves it in its plane.
        let d = m.points[7].sub(orig[7]);
        assert!(d.length() < 1e-4 && d.z.abs() < 1e-12, "{d:?}");
        m.validate().unwrap();
    }

    /// V&V (gh:#1259) — the pipeline option, and `optimiseLayer`'s and
    /// `meshOptimizer`'s hooks reached. On the 48-sided pipe (R 10 mm, L
    /// 60 mm) at h 6 mm, `MedianPerPoint` + `Corners`, bare: with
    /// `untangle_surface` off no note mentions `untangleSurface`; with it
    /// on the stage reports, and the mesh the pipeline returns is valid
    /// (closed, no inverted cell: `keep_if_ok` guarantees it, either the
    /// smoothed capture or the mesh before it). Predicted (2026-11-05,
    /// before the first run): the note shows the inverted count falling
    /// but **not to 0** (550 of the points are reflex corners of the
    /// dual's non-convex boundary polygons, gh:#1258, and in-plane
    /// smoothing of the points does not straighten a median-dual face's
    /// midpoint corners). `mesh_optimizer::optimize_surface` on that mesh
    /// keeps it closed. `CfMeshOptimise` with `optimise_surface` on
    /// returns a valid mesh with layers.
    #[test]
    fn untangle_surface_option_is_off_by_default_and_runs() {
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.06, 48);
        let o = TetDualOptions { cell_size: 0.006, n_layers: 0, dual_kind: DualKind::MedianPerPoint, dual_feature_points: FeaturePointRule::Corners, ..Default::default() };
        assert!(!o.untangle_surface);
        let (_, rep0) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
        assert!(rep0.stage_notes.iter().all(|n| !n.contains("untangleSurface")));
        let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &TetDualOptions { untangle_surface: true, ..o.clone() }).unwrap();
        println!("{:?}", rep.stage_notes);
        assert!(rep.stage_notes.iter().any(|n| n.starts_with("untangleSurface")));
        m.validate().unwrap();
        assert_eq!(crate::checks::check_quality(&m).n_negative_volume_cells, 0);

        let given = crate::surface_mapper::boundary_face_regions(&m, &p, &t, &regions);
        let mut m2 = m.clone();
        let r = crate::mesh_optimizer::optimize_surface(&mut m2, &p, &t, &regions, &given);
        println!("meshOptimizer::optimizeSurface: {r:?}; valid {:?}", m2.validate().is_ok());

        let mut lo = o.clone();
        lo.n_layers = 1;
        lo.first_layer_thickness = 3e-4;
        lo.layer_patches = vec!["walls".into()];
        lo.layer_strategy = crate::layers::LayerQualityStrategy::CfMeshOptimise;
        lo.cfmesh_layer_optimisation.optimise_surface = true;
        let (ml, repl) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &lo).unwrap();
        println!("CfMeshOptimise + optimise_surface: {} cells; {:?}", ml.cell_count(), repl.stage_notes);
        ml.validate().unwrap();
    }

    /// Projected inlet area |Σ S_f·ẑ| over the `inlet` patch, / πR².
    #[cfg(feature = "cell-centre-dual")]
    fn inlet_fraction(m: &VolumeMesh, r: f64) -> f64 {
        let q = m.patches.iter().find(|q| q.name == "inlet").unwrap();
        let a: f64 = (q.start_face..q.start_face + q.n_faces).map(|f| m.face_area_vector(f).z).sum();
        a.abs() / (std::f64::consts::PI * r * r)
    }

    /// Differential (gh:#1259), `--ignored`: `untangleSurface` (the
    /// `untangle_surface` route) against `revert_tangled` (the default
    /// capture) on the 24- and 48-sided pipes (R 10 mm, L 60 mm) and the
    /// Foam Studio pipe (48-sided, L 200 mm) at h 4, 5 and 6 mm,
    /// `CellCentre` + `Corners`, bare. Prints per case: boundary points
    /// left unmapped (reverted; the untangle route reverts none, but the
    /// pipeline keeps the uncaptured mesh if the result is invalid),
    /// whether the capture was kept, the projected inlet area / πR², cells
    /// with a non-positive pyramid, the inverted-point counts, and time.
    /// Predictions and results: the gh:#1259 section of
    /// `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
    #[cfg(feature = "cell-centre-dual")]
    #[test]
    #[ignore]
    fn diagnose_untangle_surface_differential() {
        let cases: [(&str, usize, f64); 3] = [("pipe24 L60", 24, 0.06), ("pipe48 L60", 48, 0.06), ("foam studio L200", 48, 0.2)];
        let hs: Vec<f64> = std::env::var("CFMESH_HS").ok().map_or(vec![0.004, 0.005, 0.006], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
        // CFMESH_CE=1: correct_edges on in both routes; CFMESH_CASE: only
        // the cases whose name contains it.
        let ce = std::env::var("CFMESH_CE").as_deref() == Ok("1");
        let only = std::env::var("CFMESH_CASE").ok();
        for (name, n_seg, l) in cases {
            if only.as_deref().is_some_and(|c| !name.contains(c)) {
                continue;
            }
            let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, l, n_seg);
            for &h in &hs {
                for untangle in [false, true] {
                    let o = TetDualOptions {
                        cell_size: h,
                        n_layers: 0,
                        dual_kind: DualKind::CellCentre,
                        dual_feature_points: FeaturePointRule::Corners,
                        untangle_surface: untangle,
                        correct_edges: ce,
                        ..Default::default()
                    };
                    let t0 = std::time::Instant::now();
                    let (m, rep) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
                    let secs = t0.elapsed().as_secs_f64();
                    let unmapped = rep
                        .stage_notes
                        .iter()
                        .find_map(|n| n.strip_prefix("feature-edge capture: ").and_then(|r| r.split_whitespace().next()).and_then(|x| x.parse::<usize>().ok()))
                        .unwrap_or(0);
                    let skipped = rep.stage_notes.iter().any(|n| n.contains("capture") && n.contains("skipped"));
                    let npyr = crate::surface_mapper::cells_with_nonpositive_pyramids(&m).len();
                    let e = SurfaceEngine::new(&m);
                    let inv = check_inverted_vertices(&e, &SurfacePartitioner::new(&e), &m.points, None).len();
                    let note = rep.stage_notes.iter().find(|n| n.starts_with("untangleSurface")).cloned().unwrap_or_default();
                    println!(
                        "{name} h {:.0} {}{}: {} cells, capture {}, unmapped {unmapped}, inlet/piR2 {:.5}, V/piR2L {:.5}, nonpos pyramids {npyr}, inverted pts {inv}, NO max {:.1}, {secs:.1} s; {note}",
                        h * 1e3,
                        if untangle { "untangleSurface" } else { "revert_tangled" },
                        if ce { " +correct_edges" } else { "" },
                        m.cell_count(),
                        if skipped { "REJECTED" } else { "kept" },
                        inlet_fraction(&m, 0.01),
                        m.total_volume() / (std::f64::consts::PI * 1e-4 * l),
                        crate::checks::check_quality(&m).max_non_orthogonality_deg,
                    );
                }
            }
        }
    }
}

/// Diagnostic images for gh:#1259 (`--ignored`): the inlet patch of the
/// Foam Studio pipe (48-sided, R 10 mm, L 200 mm), `CellCentre` +
/// `Corners`, bare, at h 4, 5 and 6 mm, under `revert_tangled` and
/// `untangleSurface`, drawn from the `VolumeMesh` the pipeline returns:
/// every inlet face's edges in x-y (black), the 48-gon of the input
/// surface (red), reverted / off-plane points marked (blue: an inlet point
/// with |z| > 1e-9 m). Two views per mesh: the whole inlet and a rim
/// close-up (x 6–10.5 mm, y −2.25–2.25 mm). PNGs are written to
/// `CFMESH_IMG_DIR` (default: the gh:#1259 folder of the V&V record).
///
/// The `dhoby-ghaut` renderers of gh:#1241 were not used: building the
/// Foam Studio example filled the shared disk (2026-11-05). These images
/// are drawn by this test with a small uncompressed PNG writer; CLI-like
/// glue, the drawing itself is not asserted.
#[cfg(all(test, feature = "cell-centre-dual"))]
pub(crate) mod render_gh1259 {
    use super::*;
    use crate::dual::{DualKind, FeaturePointRule};
    use crate::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
    use crate::volume_mesh::VolumeMesh;
    use petir::mathf::RealMath;

    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }

    /// An 8-bit RGB PNG with stored (uncompressed) deflate blocks.
    pub(crate) fn write_png(path: &std::path::Path, w: usize, h: usize, rgb: &[u8]) {
        let mut raw = Vec::with_capacity(h * (3 * w + 1));
        for y in 0..h {
            raw.push(0u8);
            raw.extend_from_slice(&rgb[3 * w * y..3 * w * (y + 1)]);
        }
        let mut z = vec![0x78u8, 0x01];
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        let mut chunks = raw.chunks(65535).peekable();
        while let Some(ch) = chunks.next() {
            z.push(u8::from(chunks.peek().is_none()));
            let n = ch.len() as u16;
            z.extend_from_slice(&n.to_le_bytes());
            z.extend_from_slice(&(!n).to_le_bytes());
            z.extend_from_slice(ch);
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let mut chunk = |ty: &[u8], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut td = ty.to_vec();
            td.extend_from_slice(data);
            out.extend_from_slice(&td);
            out.extend_from_slice(&crc32(&td).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        std::fs::write(path, out).unwrap();
    }

    struct Canvas {
        w: usize,
        h: usize,
        px: Vec<u8>,
        x0: f64,
        y0: f64,
        span: f64,
    }

    impl Canvas {
        fn new(w: usize, h: usize, x0: f64, y0: f64, span: f64) -> Self {
            Canvas { w, h, px: vec![255; 3 * w * h], x0, y0, span }
        }
        fn to_px(&self, x: f64, y: f64) -> (f64, f64) {
            ((x - self.x0) / self.span * self.w as f64, (1.0 - (y - self.y0) / self.span) * self.h as f64)
        }
        fn dot(&mut self, x: i64, y: i64, c: [u8; 3]) {
            if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
                let i = 3 * (y as usize * self.w + x as usize);
                self.px[i..i + 3].copy_from_slice(&c);
            }
        }
        fn line(&mut self, a: Vec3, b: Vec3, c: [u8; 3]) {
            let (p, q) = (self.to_px(a.x, a.y), self.to_px(b.x, b.y));
            let n = ((q.0 - p.0).abs().max((q.1 - p.1).abs()) as usize).clamp(1, 20_000);
            for i in 0..=n {
                let t = i as f64 / n as f64;
                self.dot((p.0 + t * (q.0 - p.0)) as i64, (p.1 + t * (q.1 - p.1)) as i64, c);
            }
        }
        fn mark(&mut self, a: Vec3, c: [u8; 3]) {
            let p = self.to_px(a.x, a.y);
            for dx in -3..=3 {
                for dy in -3..=3 {
                    self.dot(p.0 as i64 + dx, p.1 as i64 + dy, c);
                }
            }
        }
    }

    pub(crate) fn draw(m: &VolumeMesh, path: &std::path::Path, x0: f64, y0: f64, span: f64) {
        let mut c = Canvas::new(900, 900, x0, y0, span);
        for k in 0..48 {
            let (a0, a1) = (k as f64 * std::f64::consts::TAU / 48.0, (k + 1) as f64 * std::f64::consts::TAU / 48.0);
            c.line(Vec3::new(0.01 * a0.r_cos(), 0.01 * a0.r_sin(), 0.0), Vec3::new(0.01 * a1.r_cos(), 0.01 * a1.r_sin(), 0.0), [220, 30, 30]);
        }
        let q = m.patches.iter().find(|q| q.name == "inlet").unwrap();
        for f in q.start_face..q.start_face + q.n_faces {
            let r = &m.faces[f];
            for i in 0..r.len() {
                c.line(m.points[r[i]], m.points[r[(i + 1) % r.len()]], [0, 0, 0]);
            }
        }
        for f in q.start_face..q.start_face + q.n_faces {
            for &v in &m.faces[f] {
                if m.points[v].z.abs() > 1e-9 {
                    c.mark(m.points[v], [30, 90, 230]);
                }
            }
        }
        write_png(path, c.w, c.h, &c.px);
    }

    #[test]
    #[ignore]
    fn render_untangle_surface_inlets() {
        let dir = std::env::var("CFMESH_IMG_DIR").unwrap_or_else(|_| {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/gh1259").to_string()
        });
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (p, t, regions) = crate::pipeline::tests::pipe_surface(0.01, 0.2, 48);
        for h_mm in [4usize, 5, 6] {
            for (untangle, tag) in [(false, "revert"), (true, "untangle")] {
                let o = TetDualOptions {
                    cell_size: h_mm as f64 * 1e-3,
                    n_layers: 0,
                    dual_kind: DualKind::CellCentre,
                    dual_feature_points: FeaturePointRule::Corners,
                    untangle_surface: untangle,
                    ..Default::default()
                };
                let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &regions, &o).unwrap();
                draw(&m, &dir.join(format!("inlet_{tag}_h{h_mm}.png")), -0.0105, -0.0105, 0.021);
                draw(&m, &dir.join(format!("inlet_{tag}_h{h_mm}_rim.png")), 0.006, -0.00225, 0.0045);
                println!("wrote inlet_{tag}_h{h_mm}(.png, _rim.png)");
            }
        }
    }
}
