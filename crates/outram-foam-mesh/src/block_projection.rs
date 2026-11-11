// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM-dev 852527377c7e (OpenFOAM Foundation),
// GPL-3.0-or-later:
//   src/mesh/blockMesh/blockVertices/projectVertex,
//   src/mesh/blockMesh/blockEdges/{projectEdge,projectCurveEdge},
//   src/mesh/blockMesh/blockFaces/{blockFace,projectFace,sweepFace},
//   src/mesh/blockMesh/blocks/block/blockCreate.C (face-correction passes),
//   src/mesh/blockMesh/blockDescriptor/blockDescriptor{.C,I.H},
//   src/meshTools/searchableSurfaces/{sphere,cylinder,plane,
//     searchableSurfacesQueries},
//   src/OpenFOAM/meshes/primitiveShapes/plane (planeIntersect),
//   src/OpenFOAM/interpolations/.../linearInterpolationWeights.
// The OpenFOAM.com v2506 face-projection loop is kept beside it
// ([`ProjectionFlavour::Esi`]) so a v2506 `blockMesh` binary can be compared
// bit for bit (gh:#1462).

//! `blockMesh` projection: the `geometry` dictionary (searchable surfaces),
//! `project` vertices, `project` / `projectCurve` edges, `project` and
//! `sweep` block faces, and the curved-face correction passes of
//! `block::createPoints` (gh:#1462, epic gh:#1458).
//!
//! ## Scope
//!
//! Searchable surfaces: `sphere` (`searchableSphere`), `cylinder`
//! (`searchableCylinder`) and `plane` (`searchablePlane`), the shapes the
//! `mesh/blockMesh/{sphere7ProjectedEdges,pipe}` tutorials use besides
//! `extrudedCircle` and `plate`. Any other `type` (including `triSurface`) is
//! refused with [`MeshError::NotImplemented`] naming it; nothing is skipped.
//!
//! ## Dev against v2506
//!
//! The specification is OpenFOAM-dev `852527377c7e`. Its `projectFace` loop
//! stops after adding the i residual and testing the combined residual; the
//! v2506 loop projects, tests the previous residual and stops on a projected
//! point. Both are implemented ([`ProjectionFlavour`]); the default is dev.
//! The multi-surface `findNearest` is dev's (alternate surfaces, intersect the
//! tangent planes); v2506 additionally clips the step, see
//! `verification_and_validation/block_mesh_projection.md`.
//!
//! Coordinates are the raw dictionary coordinates (before `convertToMeters`):
//! upstream projects before scaling.
//!
//! Transcendentals (`sin`, `cos`, `asin` in `sweep`) go through PETIR
//! `RealMath`; `sqrt` is exact.

use std::sync::Arc;

use petir::mathf::RealMath;

use outram_foam_basic_lib::primitives::Vector3;

use crate::block_topology::same_vertices;
use crate::MeshError;

/// OpenFOAM `small` for `double` (`numeric_limits<double>::epsilon()`).
const SMALL: f64 = f64::EPSILON;
/// OpenFOAM `great` (`1/small`).
const GREAT: f64 = 1.0 / f64::EPSILON;
/// OpenFOAM `vGreat` (`max()/10`).
const V_GREAT: f64 = f64::MAX / 10.0;
/// OpenFOAM `rootSmall` (`sqrt(small)`).
const ROOT_SMALL: f64 = 1.490_116_119_384_765_6e-8;

/// `Foam::normalised(v)`: `v/|v|`, zero for a zero vector.
fn normalised(v: Vector3) -> Vector3 {
    let m = v.mag();
    if m > 0.0 {
        v / m
    } else {
        Vector3::ZERO
    }
}

fn comp(v: Vector3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn set_comp(v: &mut Vector3, i: usize, value: f64) {
    match i {
        0 => v.x = value,
        1 => v.y = value,
        _ => v.z = value,
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  searchable surfaces
// ───────────────────────────────────────────────────────────────────────────

/// A searchable surface (enum dispatch, no trait objects).
#[derive(Debug, Clone, PartialEq)]
pub enum SearchableSurface {
    /// `searchableSurfaces::sphere`.
    Sphere {
        /// Centre.
        centre: Vector3,
        /// Radius.
        radius: f64,
    },
    /// `searchableSurfaces::cylinder` (finite, with end caps).
    Cylinder {
        /// Axis start.
        point1: Vector3,
        /// Axis end.
        point2: Vector3,
        /// Radius.
        radius: f64,
    },
    /// `searchableSurfaces::plane` (infinite).
    Plane {
        /// A point on the plane.
        point: Vector3,
        /// Unit normal.
        normal: Vector3,
    },
}

impl SearchableSurface {
    /// `Foam::plane(basePoint, normalVector)`: the normal is normalised.
    pub fn plane(point: Vector3, normal: Vector3) -> Self {
        SearchableSurface::Plane {
            point,
            normal: normalised(normal),
        }
    }

    /// `findNearest(sample, nearestDistSqr)` for one sample: the nearest point
    /// on the surface when it lies within `sqrt(dist_sqr)`, else `None`.
    pub fn find_nearest(&self, sample: Vector3, dist_sqr: f64, flavour: ProjectionFlavour) -> Option<Vector3> {
        match self {
            SearchableSurface::Sphere { centre, radius } => {
                let n = sample - *centre;
                let mag_n = n.mag();
                let d = mag_n - *radius;
                if dist_sqr >= d * d {
                    if mag_n < f64::MIN_POSITIVE.sqrt() {
                        Some(*centre + Vector3::new(1.0, 0.0, 0.0) * *radius)
                    } else if flavour == ProjectionFlavour::Esi {
                        Some(*centre + (n * *radius) / mag_n)
                    } else {
                        Some(*centre + n / mag_n * *radius)
                    }
                } else {
                    None
                }
            }
            SearchableSurface::Cylinder {
                point1,
                point2,
                radius,
            } => {
                let mag_dir = (*point2 - *point1).mag();
                let unit_dir = (*point2 - *point1) / mag_dir;
                let mut v = sample - *point1;
                let parallel = v.dot(unit_dir);
                v = v - unit_dir * parallel;
                let mag_v = v.mag();
                if mag_v < f64::MIN_POSITIVE.sqrt() {
                    v = Vector3::ZERO;
                } else {
                    v = v / mag_v;
                }
                let p = if parallel <= 0.0 {
                    *point1 + v * mag_v.min(*radius)
                } else if parallel >= mag_dir {
                    *point2 + v * mag_v.min(*radius)
                } else {
                    let cyl_pt = if mag_v < f64::MIN_POSITIVE.sqrt() {
                        let mut e1 = Vector3::new(1.0, 0.0, 0.0).cross(unit_dir);
                        let mut mag_e1 = e1.mag();
                        if mag_e1 < SMALL {
                            e1 = Vector3::new(0.0, 1.0, 0.0).cross(unit_dir);
                            mag_e1 = e1.mag();
                        }
                        e1 = e1 / mag_e1;
                        sample + e1 * *radius
                    } else {
                        sample + v * (*radius - mag_v)
                    };
                    let end_pt = if parallel < 0.5 * mag_dir {
                        *point1 + v * mag_v.min(*radius)
                    } else {
                        *point2 + v * mag_v.min(*radius)
                    };
                    if (sample - cyl_pt).mag_sqr() < (sample - end_pt).mag_sqr() {
                        cyl_pt
                    } else {
                        end_pt
                    }
                };
                if (sample - p).mag_sqr() < dist_sqr {
                    Some(p)
                } else {
                    None
                }
            }
            SearchableSurface::Plane { point, normal } => {
                let signed = (sample - *point).dot(*normal);
                let p = sample - *normal * signed;
                if (sample - p).mag_sqr() > dist_sqr {
                    None
                } else {
                    Some(p)
                }
            }
        }
    }

    /// `getNormal` at a point returned by [`find_nearest`](Self::find_nearest).
    pub fn normal_at(&self, hit: Vector3, flavour: ProjectionFlavour) -> Vector3 {
        match self {
            SearchableSurface::Sphere { centre, .. } => {
                let n = hit - *centre;
                if flavour == ProjectionFlavour::Esi {
                    return normalised(n);
                }
                n / (n.mag() + f64::MIN_POSITIVE)
            }
            SearchableSurface::Cylinder {
                point1,
                point2,
                radius,
            } => {
                let mag_dir = (*point2 - *point1).mag();
                let unit_dir = (*point2 - *point1) / mag_dir;
                let mut v = hit - *point1;
                let parallel = v.dot(unit_dir);
                v = v - unit_dir * parallel;
                let mag_v = v.mag();
                if parallel <= 0.0 {
                    if (mag_v - *radius) < parallel.abs() {
                        -unit_dir
                    } else {
                        v / mag_v
                    }
                } else if parallel <= 0.5 * mag_dir {
                    if mag_v >= *radius || (*radius - mag_v) < parallel {
                        v / mag_v
                    } else {
                        -unit_dir
                    }
                } else if parallel <= mag_dir {
                    if mag_v >= *radius || (*radius - mag_v) < (mag_dir - parallel) {
                        v / mag_v
                    } else {
                        unit_dir
                    }
                } else if (mag_v - *radius) < (parallel - mag_dir) {
                    unit_dir
                } else {
                    v / mag_v
                }
            }
            SearchableSurface::Plane { normal, .. } => *normal,
        }
    }

    /// `bounds()`: `(min, max)`; the plane's are `-+vGreat` except along an
    /// axis-aligned normal (`plane::calcBounds`).
    pub fn bounds(&self) -> (Vector3, Vector3) {
        match self {
            SearchableSurface::Sphere { centre, radius } => {
                let r = Vector3::ONE * *radius;
                (*centre - r, *centre + r)
            }
            SearchableSurface::Cylinder {
                point1,
                point2,
                radius,
            } => {
                let unit_dir = (*point2 - *point1) / (*point2 - *point1).mag();
                let kr = Vector3::new(
                    (unit_dir.y * unit_dir.y + unit_dir.z * unit_dir.z).sqrt(),
                    (unit_dir.x * unit_dir.x + unit_dir.z * unit_dir.z).sqrt(),
                    (unit_dir.x * unit_dir.x + unit_dir.y * unit_dir.y).sqrt(),
                ) * *radius;
                let lo = *point1 - kr;
                let hi = *point1 + kr;
                let lo2 = *point2 - kr;
                let hi2 = *point2 + kr;
                (
                    Vector3::new(lo.x.min(lo2.x), lo.y.min(lo2.y), lo.z.min(lo2.z)),
                    Vector3::new(hi.x.max(hi2.x), hi.y.max(hi2.y), hi.z.max(hi2.z)),
                )
            }
            SearchableSurface::Plane { point, normal } => {
                let mut lo = Vector3::new(-V_GREAT, -V_GREAT, -V_GREAT);
                let mut hi = Vector3::new(V_GREAT, V_GREAT, V_GREAT);
                for dir in 0..3 {
                    if comp(*normal, dir) > 1.0 - SMALL {
                        set_comp(&mut hi, dir, comp(*point, dir));
                        break;
                    }
                    if comp(*normal, dir) < SMALL - 1.0 {
                        set_comp(&mut lo, dir, comp(*point, dir));
                        break;
                    }
                }
                (lo, hi)
            }
        }
    }
}

/// `searchableSurfaceList`: the named surfaces of the `geometry` dictionary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Geometry {
    /// `(name, surface)` in dictionary order.
    pub surfaces: Vec<(String, SearchableSurface)>,
    /// Which OpenFOAM's arithmetic and loops to follow.
    pub flavour: ProjectionFlavour,
}

/// `plane::ray`: a reference point and a direction.
struct PlaneRay {
    ref_point: Vector3,
    dir: Vector3,
}

/// `plane::planeIntersect`: the line where the plane `(p1, n1)` meets `(p2, n2)`.
fn plane_intersect(p1: Vector3, n1: Vector3, p2: Vector3, n2: Vector3) -> PlaneRay {
    let n1p1 = n1.dot(p1);
    let n2p2 = n2.dot(p2);
    let dir = n1.cross(n2);
    let mag_x = dir.x.abs();
    let mag_y = dir.y.abs();
    let mag_z = dir.z.abs();
    let (i_zero, i1, i2) = if mag_x > mag_y {
        if mag_x > mag_z {
            (0, 1, 2)
        } else {
            (2, 0, 1)
        }
    } else if mag_y > mag_z {
        (1, 2, 0)
    } else {
        (2, 0, 1)
    };
    let mut pt = Vector3::ZERO;
    set_comp(&mut pt, i_zero, 0.0);
    let c = |v: Vector3, i: usize| comp(v, i);
    set_comp(
        &mut pt,
        i1,
        (c(n2, i2) * n1p1 - c(n1, i2) * n2p2) / (c(n1, i1) * c(n2, i2) - c(n2, i1) * c(n1, i2)),
    );
    set_comp(
        &mut pt,
        i2,
        (c(n2, i1) * n1p1 - c(n1, i1) * n2p2) / (c(n1, i2) * c(n2, i1) - c(n1, i1) * c(n2, i2)),
    );
    PlaneRay { ref_point: pt, dir }
}

/// Dev: `(I - sqr(n)) & (refPoint - near)` with `n` the unit line direction.
fn onto_line(r: &PlaneRay, near: Vector3) -> Vector3 {
    let nn = r.dir / r.dir.mag();
    let v = r.ref_point - near;
    // entry by entry as the tensor product `(I - sqr(n)) & v`
    let m = |a: usize, b: usize| (if a == b { 1.0 } else { 0.0 }) - comp(nn, a) * comp(nn, b);
    Vector3::new(
        m(0, 0) * v.x + m(0, 1) * v.y + m(0, 2) * v.z,
        m(1, 0) * v.x + m(1, 1) * v.y + m(1, 2) * v.z,
        m(2, 0) * v.x + m(2, 1) * v.y + m(2, 2) * v.z,
    )
}

/// v2506: `d = refPoint - near; d.removeCollinear(n)`.
fn onto_line_esi(r: &PlaneRay, near: Vector3) -> Vector3 {
    let nn = r.dir / r.dir.mag();
    let d = r.ref_point - near;
    d - nn * d.dot(nn)
}

impl Geometry {
    /// `findSurfaceID`.
    pub fn find_surface_id(&self, name: &str) -> Option<usize> {
        self.surfaces.iter().position(|(n, _)| n == name)
    }

    /// Ids of the listed surface names.
    ///
    /// # Errors
    /// [`MeshError::DictParse`] `Cannot find surface <name> in geometry`, or
    /// for an empty list.
    pub fn surface_ids(&self, names: &[String]) -> Result<Vec<usize>, MeshError> {
        if names.is_empty() {
            return Err(MeshError::DictParse(
                "a projection needs at least one surface".into(),
            ));
        }
        names
            .iter()
            .map(|n| {
                self.find_surface_id(n).ok_or_else(|| {
                    MeshError::DictParse(format!("Cannot find surface {n} in geometry"))
                })
            })
            .collect()
    }

    /// `searchableSurfacesQueries::bounds`.
    pub fn bounds(&self, ids: &[usize]) -> (Vector3, Vector3) {
        let mut lo = Vector3::new(V_GREAT, V_GREAT, V_GREAT);
        let mut hi = Vector3::new(-V_GREAT, -V_GREAT, -V_GREAT);
        for &id in ids {
            let (a, b) = self.surfaces[id].1.bounds();
            lo = Vector3::new(lo.x.min(a.x), lo.y.min(a.y), lo.z.min(a.z));
            hi = Vector3::new(hi.x.max(b.x), hi.y.max(b.y), hi.z.max(b.z));
        }
        (lo, hi)
    }

    /// `searchableSurfacesQueries::findNearest` (multi-surface form, dev): the
    /// nearest point on the first surface, then `n_iter` alternations over the
    /// remaining surfaces, each moving onto the line where the tangent planes
    /// meet. Points with no hit keep their start position.
    pub fn find_nearest(
        &self,
        ids: &[usize],
        start: &[Vector3],
        dist_sqr: &[f64],
        n_iter: usize,
    ) -> Vec<Vector3> {
        let n = start.len();
        let fl = self.flavour;
        let mut near = start.to_vec();
        let mut not_hit = vec![true; n];
        let mut normal = vec![Vector3::ZERO; n];
        let first = &self.surfaces[ids[0]].1;
        for i in 0..n {
            if let Some(p) = first.find_nearest(start[i], dist_sqr[i], fl) {
                not_hit[i] = false;
                near[i] = p;
                normal[i] = first.normal_at(p, fl);
            }
        }
        if ids.len() == 1 {
            return near;
        }
        let mut surfi = 1usize;
        for _ in 0..n_iter {
            let s = &self.surfaces[ids[surfi]].1;
            let info: Vec<Option<Vector3>> = (0..n)
                .map(|i| s.find_nearest(near[i], dist_sqr[i], fl))
                .collect();
            for i in 0..n {
                let Some(near1) = info[i] else { continue };
                let normal1 = s.normal_at(near1, fl);
                match fl {
                    ProjectionFlavour::Dev => {
                        if not_hit[i] {
                            not_hit[i] = false;
                            near[i] = near1;
                            normal[i] = normal1;
                        } else if normal[i].dot(normal1).abs() < 1.0 - ROOT_SMALL {
                            let r = plane_intersect(
                                near[i],
                                normalised(normal[i]),
                                near1,
                                normalised(normal1),
                            );
                            let d = onto_line(&r, near[i]);
                            near[i] += d;
                            normal[i] = normal1;
                        }
                    }
                    ProjectionFlavour::Esi => {
                        if normal[i] != Vector3::ZERO {
                            if normal[i].dot(normal1).abs() < 1.0 - 1e-6 {
                                let r = plane_intersect(near[i], normal[i], near1, normal1);
                                let mut d = onto_line_esi(&r, near[i]);
                                let mag_d = d.mag();
                                // v2506 SMALL is 1e-15 (dev: epsilon).
                                if mag_d > 1e-15 {
                                    let max_dist = dist_sqr[i].sqrt();
                                    if mag_d > max_dist {
                                        d = d / mag_d;
                                        d = d * max_dist;
                                    }
                                    near[i] += d;
                                    normal[i] = normal1;
                                }
                            }
                        } else {
                            near[i] = near1;
                            normal[i] = normal1;
                        }
                    }
                }
            }
            surfi = (surfi + 1) % ids.len();
        }
        near
    }

    /// `blockVertices::projectVertex::operator point()`.
    pub fn project_vertex(&self, ids: &[usize], p: Vector3) -> Vector3 {
        let (mut lo, mut hi) = self.bounds(ids);
        lo = Vector3::new(lo.x.max(-GREAT), lo.y.max(-GREAT), lo.z.max(-GREAT));
        hi = Vector3::new(hi.x.min(GREAT), hi.y.min(GREAT), hi.z.min(GREAT));
        let span = hi - lo;
        self.find_nearest(ids, &[p], &[span.mag_sqr()], 20)[0]
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  linearInterpolationWeights
// ───────────────────────────────────────────────────────────────────────────

/// `linearInterpolationWeights` over monotone `samples`, with the cached
/// interval index of the original.
struct LinearWeights<'a> {
    samples: &'a [f64],
    index: i64,
    flavour: ProjectionFlavour,
}

impl<'a> LinearWeights<'a> {
    fn new(samples: &'a [f64], flavour: ProjectionFlavour) -> Self {
        Self {
            samples,
            index: -1,
            flavour,
        }
    }

    fn update_index(&mut self, t: f64) {
        let n = self.samples.len() as i64;
        if self.flavour == ProjectionFlavour::Esi {
            // v2506: the cached slot holds if samples[idx] <= t <= samples[idx+1].
            let ok = self.index >= 0
                && self.index < n
                && self.samples[self.index as usize] <= t
                && (self.index == n - 1 || t <= self.samples[self.index as usize + 1]);
            if !ok {
                self.index = self.samples.partition_point(|&s| s < t) as i64 - 1;
            }
            return;
        }
        let valid = (self.index == -1 && t <= self.samples[0])
            || (self.index >= 0
                && self.index < n - 1
                && t >= self.samples[self.index as usize]
                && t <= self.samples[self.index as usize + 1])
            || (self.index == n - 1 && t >= self.samples[(n - 1) as usize]);
        if valid {
            return;
        }
        // findLower: last index with samples[i] < t, or -1.
        self.index = self.samples.partition_point(|&s| s < t) as i64 - 1;
    }

    /// `valueWeights`: `(index, weight)` pairs.
    fn value_weights(&mut self, t: f64) -> ([(usize, f64); 2], usize) {
        self.update_index(t);
        let n = self.samples.len() as i64;
        if self.index == -1 {
            ([(0, 1.0), (0, 0.0)], 1)
        } else if self.index == n - 1 {
            ([((n - 1) as usize, 1.0), (0, 0.0)], 1)
        } else {
            let i = self.index as usize;
            if self.flavour == ProjectionFlavour::Esi {
                let (t0, t1) = (self.samples[i], self.samples[i + 1]);
                let w0 = (t1 - t) / (t1 - t0);
                return ([(i, w0), (i + 1, 1.0 - w0)], 2);
            }
            let f = (t - self.samples[i]) / (self.samples[i + 1] - self.samples[i]);
            ([(i, 1.0 - f), (i + 1, f)], 2)
        }
    }
}

/// Normalised cumulative chord length of a polyline of points.
fn chord_lambdas(points: &[Vector3]) -> Vec<f64> {
    let mut l = vec![0.0; points.len()];
    for i in 1..points.len() {
        l[i] = l[i - 1] + (points[i] - points[i - 1]).mag();
    }
    let last = l[l.len() - 1];
    for v in l.iter_mut() {
        *v /= last;
    }
    l
}

// ───────────────────────────────────────────────────────────────────────────
//  projectEdge / projectCurveEdge
// ───────────────────────────────────────────────────────────────────────────

/// A `project` or `projectCurve` block edge: the straight line between the two
/// vertices projected onto the listed surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectEdge {
    geometry: Arc<Geometry>,
    surfaces: Vec<usize>,
    p0: Vector3,
    p1: Vector3,
}

impl ProjectEdge {
    /// Build the edge. `projectCurve` shares the algorithm of `project` when
    /// no `extrudedCircle` is among the surfaces (the only one it special-cases,
    /// and not ported).
    pub fn new(geometry: Arc<Geometry>, surfaces: Vec<usize>, p0: Vector3, p1: Vector3) -> Self {
        Self {
            geometry,
            surfaces,
            p0,
            p1,
        }
    }

    /// `projectEdge::position(lambda)`: the straight-line point projected once,
    /// except at the ends.
    pub fn position(&self, lambda: f64) -> Vector3 {
        let start = self.p0 + (self.p1 - self.p0) * lambda;
        if lambda >= SMALL && lambda < 1.0 - SMALL {
            let dist_sqr = (self.p1 - self.p0).mag_sqr();
            self.geometry
                .find_nearest(&self.surfaces, &[start], &[dist_sqr], 20)[0]
        } else {
            start
        }
    }

    /// `projectEdge::position(const scalarList&)`.
    pub fn positions(&self, lambdas: &[f64]) -> Vec<Vector3> {
        self.positions_counted(lambdas).0
    }

    /// As [`positions`](Self::positions), also returning the number of
    /// projection passes made (for the V&V record).
    pub fn positions_counted(&self, lambdas: &[f64]) -> (Vec<Vector3>, usize) {
        let d = self.p1 - self.p0;
        let mut points: Vec<Vector3> = lambdas.iter().map(|&l| self.p0 + d * l).collect();
        let max_iter = 10;
        let rel_tol = 0.1;
        let abs_tol = 1e-4;
        let mut initial_residual = 0.0;
        let mut passes = 0usize;
        let dist_sqr = vec![d.mag_sqr(); points.len()];
        for iter in 0..max_iter {
            passes += 1;
            let start = points.clone();
            points = self.geometry.find_nearest(&self.surfaces, &start, &dist_sqr, 20);
            if lambdas[0] < SMALL {
                points[0] = self.p0;
            }
            if lambdas[lambdas.len() - 1] > 1.0 - SMALL {
                let last = points.len() - 1;
                points[last] = self.p1;
            }
            let proj_lambdas = chord_lambdas(&points);
            let mut interp = LinearWeights::new(&proj_lambdas, self.geometry.flavour);
            let mut residual = vec![Vector3::ZERO; points.len()];
            for i in 1..points.len().saturating_sub(1) {
                let (iw, n) = interp.value_weights(lambdas[i]);
                let mut predicted = Vector3::ZERO;
                for &(idx, w) in &iw[..n] {
                    predicted += points[idx] * w;
                }
                residual[i] = predicted - points[i];
            }
            let scalar_residual: f64 = residual.iter().map(|r| r.mag()).sum();
            if scalar_residual < abs_tol * 0.5 * lambdas.len() as f64 {
                break;
            } else if iter == 0 {
                initial_residual = scalar_residual;
            } else if scalar_residual / initial_residual < rel_tol {
                break;
            }
            for (p, r) in points.iter_mut().zip(&residual) {
                *p += *r;
            }
        }
        (points, passes)
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  block faces
// ───────────────────────────────────────────────────────────────────────────

/// Which OpenFOAM's projection code to follow where dev and v2506 differ.
///
/// The differences found (gh:#1462): the `projectFace` loop, the multi-surface
/// `findNearest` (step clipping, collinearity test), the sphere's nearest point
/// (`n/magN*radius` against `radius*n/magN`, one ulp), its normal, and the
/// `linearInterpolationWeights` (`1 - f, f` against `(t1 - t)/dT, 1 - w0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionFlavour {
    /// OpenFOAM-dev `852527377c7e` (the specification, and the default).
    #[default]
    Dev,
    /// OpenFOAM.com v2506 (the binary available for comparison).
    Esi,
}

/// What a `faces` entry does.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockFaceKind {
    /// `project (v0 v1 v2 v3) surfaceName`.
    Project(String),
    /// `sweep (v0 v1 v2 v3) rotationFraction`.
    Sweep(f64),
}

/// One `faces` entry: the four vertex labels and what to do to the face.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockFace {
    /// The face's vertex labels, in the user's order.
    pub vertices: [usize; 4],
    /// The operation.
    pub kind: BlockFaceKind,
}

/// `(dir0, dir1)` density indices of each hex face (`projectFace`, `sweepFace`).
const FACE_DIRS: [(usize, usize); 6] = [(2, 1), (2, 1), (2, 0), (2, 0), (1, 0), (1, 0)];

/// Dev `projectFace::project` (and its v2506 variant) on the face points
/// `points` (`n.0 * n.1`, first index fastest), with `n = (n.first, n.second)`.
pub fn project_face(
    geometry: &Geometry,
    surface: usize,
    n: (usize, usize),
    points: &mut [Vector3],
) {
    let flavour = geometry.flavour;
    let index = |i: usize, j: usize| i + j * n.0;
    let size = points.len();
    // calcLambdas
    let mut lambda_i = vec![0.0; size];
    let mut lambda_j = vec![0.0; size];
    for i in 1..n.0 {
        for j in 1..n.1 {
            let ij = index(i, j);
            lambda_i[ij] = lambda_i[index(i - 1, j)] + (points[ij] - points[index(i - 1, j)]).mag();
            lambda_j[ij] = lambda_j[index(i, j - 1)] + (points[ij] - points[index(i, j - 1)]).mag();
        }
    }
    for i in 1..n.0 {
        let ij_last = index(i, n.1 - 1);
        for j in 1..n.1 {
            let ij = index(i, j);
            lambda_j[ij] /= lambda_j[ij_last];
        }
    }
    for j in 1..n.1 {
        let i_last_j = index(n.0 - 1, j);
        for i in 1..n.0 {
            let ij = index(i, j);
            lambda_i[ij] /= lambda_i[i_last_j];
        }
    }

    let max_iter = 10;
    let rel_tol = 0.1;
    let abs_tol = 1e-4;
    let mut initial_residual = 0.0;
    let mut i_res = 0.0;
    let mut j_res = 0.0;
    let surf = &geometry.surfaces[surface].1;

    for iter in 0..max_iter {
        // Do projection.
        let dist_sqr = (points[0] - points[size - 1]).mag_sqr();
        for p in points.iter_mut() {
            if let Some(hit) = surf.find_nearest(*p, dist_sqr, flavour) {
                *p = hit;
            }
        }

        if flavour == ProjectionFlavour::Esi
            && iter > 0
            && (initial_residual < 1e-150 || (i_res + j_res) / initial_residual < rel_tol)
        {
            break;
        }

        // Predict along i.
        let mut residual = vec![Vector3::ZERO; size];
        for j in 1..n.1 - 1 {
            let mut proj_lambdas = vec![0.0; n.0];
            for i in 1..n.0 {
                proj_lambdas[i] =
                    proj_lambdas[i - 1] + (points[index(i, j)] - points[index(i - 1, j)]).mag();
            }
            let last = proj_lambdas[n.0 - 1];
            for v in proj_lambdas.iter_mut() {
                *v /= last;
            }
            let mut interp = LinearWeights::new(&proj_lambdas, flavour);
            for i in 1..n.0 - 1 {
                let ij = index(i, j);
                let (iw, cnt) = interp.value_weights(lambda_i[ij]);
                let mut predicted = Vector3::ZERO;
                for &(idx, w) in &iw[..cnt] {
                    predicted += points[index(idx, j)] * w;
                }
                residual[ij] = predicted - points[ij];
            }
        }
        let mut scalar_residual: f64 = residual.iter().map(|r| r.mag()).sum();
        i_res = scalar_residual;
        for (p, r) in points.iter_mut().zip(&residual) {
            *p += *r;
        }

        // Predict along j.
        residual = vec![Vector3::ZERO; size];
        for i in 1..n.0 - 1 {
            let mut proj_lambdas = vec![0.0; n.1];
            for j in 1..n.1 {
                proj_lambdas[j] =
                    proj_lambdas[j - 1] + (points[index(i, j)] - points[index(i, j - 1)]).mag();
            }
            let last = proj_lambdas[n.1 - 1];
            for v in proj_lambdas.iter_mut() {
                *v /= last;
            }
            let mut interp = LinearWeights::new(&proj_lambdas, flavour);
            for j in 1..n.1 - 1 {
                let ij = index(i, j);
                let (iw, cnt) = interp.value_weights(lambda_j[ij]);
                let mut predicted = Vector3::ZERO;
                for &(idx, w) in &iw[..cnt] {
                    predicted += points[index(i, idx)] * w;
                }
                residual[ij] = predicted - points[ij];
            }
        }
        let j_sum: f64 = residual.iter().map(|r| r.mag()).sum();
        j_res = j_sum;
        scalar_residual += j_sum;

        match flavour {
            ProjectionFlavour::Dev => {
                if scalar_residual < abs_tol * lambda_i.len() as f64 {
                    break;
                } else if iter == 0 {
                    initial_residual = scalar_residual;
                } else if scalar_residual / initial_residual < rel_tol {
                    break;
                }
            }
            ProjectionFlavour::Esi => {
                if iter == 0 {
                    initial_residual = i_res + j_res;
                }
            }
        }
        for (p, r) in points.iter_mut().zip(&residual) {
            *p += *r;
        }
    }
}

// --- small dense tensors for sweepFace (row-major, OpenFOAM operation order)

type Mat = [[f64; 3]; 3];

fn mat_vec(a: &Mat, v: Vector3) -> Vector3 {
    Vector3::new(
        a[0][0] * v.x + a[0][1] * v.y + a[0][2] * v.z,
        a[1][0] * v.x + a[1][1] * v.y + a[1][2] * v.z,
        a[2][0] * v.x + a[2][1] * v.y + a[2][2] * v.z,
    )
}

fn mat_mul(a: &Mat, b: &Mat) -> Mat {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn mat_scale(a: &Mat, s: f64) -> Mat {
    let mut r = *a;
    for row in r.iter_mut() {
        for e in row.iter_mut() {
            *e *= s;
        }
    }
    r
}

/// `rotationTensor(n1, n2)`.
fn rotation_tensor(n1: Vector3, n2: Vector3) -> Mat {
    let c = n1.dot(n2);
    let n3 = n1.cross(n2);
    let mag_sqr_n3 = n3.mag_sqr();
    let delta = |i: usize, j: usize| if i == j { 1.0 } else { 0.0 };
    let mut r = [[0.0; 3]; 3];
    if mag_sqr_n3 > SMALL {
        for i in 0..3 {
            for j in 0..3 {
                r[i][j] = c * delta(i, j)
                    + (comp(n2, i) * comp(n1, j) - comp(n1, i) * comp(n2, j))
                    + (1.0 - c) * (comp(n3, i) * comp(n3, j)) / mag_sqr_n3;
            }
        }
    } else if c < 0.0 {
        // perpendicular(n1): unit vector on the smallest component, crossed.
        let mut k = 0;
        for i in 1..3 {
            if comp(n1, i).abs() < comp(n1, k).abs() {
                k = i;
            }
        }
        let mut u = Vector3::ZERO;
        set_comp(&mut u, k, 1.0);
        let n4 = u.cross(n1);
        let mag_sqr_n4 = n4.mag_sqr();
        for i in 0..3 {
            for j in 0..3 {
                r[i][j] = -delta(i, j) + 2.0 * (comp(n4, i) * comp(n4, j)) / mag_sqr_n4;
            }
        }
    } else {
        for i in 0..3 {
            r[i][i] = 1.0;
        }
    }
    r
}

/// `Ra(a, omega)`: rotation about the unit axis `a` by `omega` radians.
fn rotation_about(a: Vector3, omega: f64) -> Mat {
    let s = omega.r_sin();
    let c = omega.r_cos();
    [
        [
            a.x * a.x * (1.0 - c) + c,
            a.y * a.x * (1.0 - c) - a.z * s,
            a.x * a.z * (1.0 - c) + a.y * s,
        ],
        [
            a.x * a.y * (1.0 - c) + a.z * s,
            a.y * a.y * (1.0 - c) + c,
            a.y * a.z * (1.0 - c) - a.x * s,
        ],
        [
            a.x * a.z * (1.0 - c) - a.y * s,
            a.y * a.z * (1.0 - c) + a.x * s,
            a.z * a.z * (1.0 - c) + c,
        ],
    ]
}

/// `face::area` of a point loop (triangle fan about the average).
fn face_area(ps: &[Vector3]) -> Vector3 {
    if ps.len() == 3 {
        return (1.0 / 2.0) * (ps[1] - ps[0]).cross(ps[2] - ps[0]);
    }
    let mut avg = Vector3::ZERO;
    for p in ps {
        avg += *p;
    }
    avg = avg / ps.len() as f64;
    let mut sum = Vector3::ZERO;
    for (k, p) in ps.iter().enumerate() {
        let next = ps[(k + 1) % ps.len()];
        sum += (next - *p).cross(avg - *p);
    }
    (1.0 / 2.0) * sum
}

/// `sweepFace::project`: `points` are the face points of block face
/// `block_face_i` (point `(row, col)` at `row * n_cols + col`, transposed when
/// the user's vertex order starts along the other direction);
/// `transpose` is decided by the caller from the vertex order
/// (see [`sweep_transpose`]).
pub fn sweep_face(
    density: [usize; 3],
    block_face_i: usize,
    transpose: bool,
    rotation_fraction: f64,
    points: &mut [Vector3],
) {
    let (d0, d1) = FACE_DIRS[block_face_i];
    let mut n_rows = density[d0] + 1;
    let mut n_cols = density[d1] + 1;
    if transpose {
        std::mem::swap(&mut n_rows, &mut n_cols);
    }
    let pi = move |i: i64, j: i64| -> usize {
        let i = if i < 0 { n_rows as i64 + i } else { i } as usize;
        let j = if j < 0 { n_cols as i64 + j } else { j } as usize;
        if transpose {
            j * n_rows + i
        } else {
            i * n_cols + j
        }
    };
    let p = |pts: &[Vector3], i: i64, j: i64| pts[pi(i, j)];

    let mut d_theta_around = vec![0.0; n_rows];
    for i in 1..n_rows as i64 - 1 {
        let axisi = normalised(p(points, i, -1) - p(points, i, 0));
        // A = I - sqr(axisi)
        let a: Mat = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                (if r == c { 1.0 } else { 0.0 }) - comp(axisi, r) * comp(axisi, c)
            })
        });
        let f_prev = [
            points[pi(i, 0)],
            points[pi(i, -1)],
            points[pi(i - 1, -1)],
            points[pi(i - 1, 0)],
        ];
        let f_next = [
            points[pi(i + 1, 0)],
            points[pi(i + 1, -1)],
            points[pi(i, -1)],
            points[pi(i, 0)],
        ];
        let a_prev = face_area(&f_prev);
        let a_next = face_area(&f_next);
        d_theta_around[i as usize] = (normalised(mat_vec(&a, a_prev))
            .cross(normalised(mat_vec(&a, a_next)))
            .dot(axisi))
        .r_asin();
    }
    let mut theta0 = vec![0.0; n_rows];
    let mut theta1 = vec![0.0; n_rows];
    for i in 1..n_rows - 1 {
        theta0[i] = theta0[i - 1] + d_theta_around[i];
    }
    for i in (1..n_rows - 1).rev() {
        theta1[i] = theta1[i + 1] - d_theta_around[i];
    }

    let axis0 = normalised(p(points, 0, -1) - p(points, 0, 0));
    let axis1 = normalised(p(points, -1, -1) - p(points, -1, 0));
    for i in 1..n_rows as i64 - 1 {
        let iu = i as usize;
        let rof0 = rotation_tensor(
            normalised(p(points, 0, -1) - p(points, 0, 0)),
            normalised(p(points, i, -1) - p(points, i, 0)),
        );
        let rof1 = rotation_tensor(
            normalised(p(points, -1, -1) - p(points, -1, 0)),
            normalised(p(points, i, -1) - p(points, i, 0)),
        );
        let raround0 = rotation_about(axis0, rotation_fraction * theta0[iu]);
        let raround1 = rotation_about(axis1, rotation_fraction * theta1[iu]);
        let scale0 = (p(points, i, -1) - p(points, i, 0)).mag()
            / (p(points, 0, -1) - p(points, 0, 0)).mag();
        let scale1 = (p(points, i, -1) - p(points, i, 0)).mag()
            / (p(points, -1, -1) - p(points, -1, 0)).mag();
        let t0 = mat_mul(&rof0, &mat_scale(&raround0, scale0));
        let t1 = mat_mul(&rof1, &mat_scale(&raround1, scale1));
        for j in 1..n_cols as i64 - 1 {
            let fi = i as f64 / (n_rows as f64 - 1.0);
            let fj = j as f64 / (n_cols as f64 - 1.0);
            let v = (1.0 - fi) * (1.0 - fj)
                * (p(points, i, 0) + mat_vec(&t0, p(points, 0, j) - p(points, 0, 0)))
                + (1.0 - fi) * fj
                    * (p(points, i, -1) + mat_vec(&t0, p(points, 0, j) - p(points, 0, -1)))
                + fi * (1.0 - fj)
                    * (p(points, i, 0) + mat_vec(&t1, p(points, -1, j) - p(points, -1, 0)))
                + fi * fj
                    * (p(points, i, -1) + mat_vec(&t1, p(points, -1, j) - p(points, -1, -1)));
            points[pi(i, j)] = v;
        }
    }
}

/// The `transpose` decision of `sweepFace::project`: look along the user's face
/// loop for the first edge equal to the block face's anchor edge; transpose
/// when it is an odd edge of the loop.
///
/// `block_labels` are the block's 8 vertex labels, `block_face_i` the hex face
/// index, `user_face` the `sweep (…)` vertex labels.
pub fn sweep_transpose(block_labels: &[usize; 8], block_face_i: usize, user_face: &[usize; 4]) -> bool {
    // blockFaceAnchorEdgeis = {4, 5, 0, 1, 0, 3} into the hex edge list.
    const ANCHOR: [usize; 6] = [4, 5, 0, 1, 0, 3];
    const HEX_EDGES: [(usize, usize); 12] = [
        (0, 1),
        (3, 2),
        (7, 6),
        (4, 5),
        (0, 3),
        (1, 2),
        (5, 6),
        (4, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let (a, b) = HEX_EDGES[ANCHOR[block_face_i]];
    let (a, b) = (block_labels[a], block_labels[b]);
    for i in 0..4 {
        let (u, v) = (user_face[i], user_face[(i + 1) % 4]);
        if (u == a && v == b) || (u == b && v == a) {
            return i % 2 == 1;
        }
    }
    false
}

// ───────────────────────────────────────────────────────────────────────────
//  block::createPoints, curved-face passes
// ───────────────────────────────────────────────────────────────────────────

/// `blockDescriptor::findCurvedFaces`: for each hex face, the index of the
/// first `faces` entry covering the same four vertices.
pub fn find_curved_faces(block_labels: &[usize; 8], faces: &[BlockFace]) -> [Option<usize>; 6] {
    const HEX_FACES: [[usize; 4]; 6] = [
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
        [0, 3, 2, 1],
        [4, 5, 6, 7],
    ];
    std::array::from_fn(|bf| {
        let bl: Vec<usize> = HEX_FACES[bf].iter().map(|&k| block_labels[k]).collect();
        faces
            .iter()
            .position(|f| same_vertices(&f.vertices, &bl))
    })
}

/// The curved-face half of `block::createPoints` (`blockCreate.C`): extract the
/// six face point sets, project/sweep the curved ones
/// (`blockDescriptor::correctFacePoints`), then apply the i-, j- and k-face
/// corrections to the block points in place.
///
/// `w` are the 12 edge weight arrays (`edgesPointsWeights`), `n` the cell
/// counts, `points` the block's `(n0+1)(n1+1)(n2+1)` points, `i` fastest.
///
/// # Errors
/// [`MeshError::DictParse`] for a `project` face naming a surface that is not
/// in `geometry`.
#[allow(clippy::too_many_arguments)]
pub fn apply_curved_faces(
    geometry: &Geometry,
    faces: &[BlockFace],
    curved: &[Option<usize>; 6],
    block_labels: &[usize; 8],
    n: [usize; 3],
    w: &[Vec<f64>; 12],
    points: &mut [Vector3],
) -> Result<(), MeshError> {
    let [ni, nj, nk] = n;
    let pl = |i: usize, j: usize, k: usize| i + j * (ni + 1) + k * (ni + 1) * (nj + 1);
    // facePointLabel
    let fpl = |face: usize, a: usize, b: usize| -> usize {
        match face {
            0 | 1 => a + b * (nj + 1),
            2 | 3 => a + b * (ni + 1),
            _ => a + b * (ni + 1),
        }
    };

    // facePoints
    let mut fp: [Vec<Vector3>; 6] = [
        vec![Vector3::ZERO; (nj + 1) * (nk + 1)],
        vec![Vector3::ZERO; (nj + 1) * (nk + 1)],
        vec![Vector3::ZERO; (ni + 1) * (nk + 1)],
        vec![Vector3::ZERO; (ni + 1) * (nk + 1)],
        vec![Vector3::ZERO; (ni + 1) * (nj + 1)],
        vec![Vector3::ZERO; (ni + 1) * (nj + 1)],
    ];
    for j in 0..=nj {
        for k in 0..=nk {
            fp[0][fpl(0, j, k)] = points[pl(0, j, k)];
            fp[1][fpl(1, j, k)] = points[pl(ni, j, k)];
        }
    }
    for i in 0..=ni {
        for k in 0..=nk {
            fp[2][fpl(2, i, k)] = points[pl(i, 0, k)];
            fp[3][fpl(3, i, k)] = points[pl(i, nj, k)];
        }
    }
    for i in 0..=ni {
        for j in 0..=nj {
            fp[4][fpl(4, i, j)] = points[pl(i, j, 0)];
            fp[5][fpl(5, i, j)] = points[pl(i, j, nk)];
        }
    }

    // correctFacePoints
    for bf in 0..6 {
        let Some(fi) = curved[bf] else { continue };
        let face = &faces[fi];
        match &face.kind {
            BlockFaceKind::Project(name) => {
                let sid = geometry.find_surface_id(name).ok_or_else(|| {
                    MeshError::DictParse(format!("Cannot find surface {name} in geometry"))
                })?;
                let (d0, d1) = match bf {
                    0 | 1 => (nj, nk),
                    2 | 3 => (ni, nk),
                    _ => (ni, nj),
                };
                project_face(geometry, sid, (d0 + 1, d1 + 1), &mut fp[bf]);
            }
            BlockFaceKind::Sweep(frac) => {
                let transpose = sweep_transpose(block_labels, bf, &face.vertices);
                sweep_face(n, bf, transpose, *frac, &mut fp[bf]);
            }
        }
    }

    let flat = |i: usize, j: usize, k: usize| -> bool {
        if i == 0 && curved[0].is_none() {
            return true;
        }
        if i == ni && curved[1].is_none() {
            return true;
        }
        if j == 0 && curved[2].is_none() {
            return true;
        }
        if j == nj && curved[3].is_none() {
            return true;
        }
        if k == 0 && curved[4].is_none() {
            return true;
        }
        if k == nk && curved[5].is_none() {
            return true;
        }
        let i_end = i == 0 || i == ni;
        let j_end = j == 0 || j == nj;
        let k_end = k == 0 || k == nk;
        (i_end && j_end) || (i_end && k_end) || (j_end && k_end)
    };

    // i-faces
    for ii in 0..=ni {
        let i = (ii + 1) % (ni + 1);
        let (w0, w1, w2, w3) = (w[0][i], w[1][i], w[2][i], w[3][i]);
        for j in 0..=nj {
            let (w4, w5, w6, w7) = (w[4][j], w[5][j], w[6][j], w[7][j]);
            for k in 0..=nk {
                if flat(i, j, k) {
                    continue;
                }
                let (w8, w9, w10, w11) = (w[8][k], w[9][k], w[10][k], w[11][k]);
                let mut wf0 = (1.0 - w0) * (1.0 - w4) * (1.0 - w8)
                    + (1.0 - w1) * w4 * (1.0 - w11)
                    + (1.0 - w2) * w7 * w11
                    + (1.0 - w3) * (1.0 - w7) * w8;
                // Upstream reads `w2*w5*w10` here (not w6): kept as written.
                let mut wf1 = w0 * (1.0 - w5) * (1.0 - w9)
                    + w1 * w5 * (1.0 - w10)
                    + w2 * w5 * w10
                    + w3 * (1.0 - w6) * w9;
                let sum = wf0 + wf1;
                wf0 /= sum;
                wf1 /= sum;
                let v = pl(i, j, k);
                points[v] += (fp[0][fpl(0, j, k)] - points[pl(0, j, k)]) * wf0
                    + (fp[1][fpl(1, j, k)] - points[pl(ni, j, k)]) * wf1;
            }
        }
    }

    // j-faces
    for jj in 0..=nj {
        let j = (jj + 1) % (nj + 1);
        let (w4, w5, w6, w7) = (w[4][j], w[5][j], w[6][j], w[7][j]);
        for i in 0..=ni {
            let (w0, w1, w2, w3) = (w[0][i], w[1][i], w[2][i], w[3][i]);
            for k in 0..=nk {
                if flat(i, j, k) {
                    continue;
                }
                let (w8, w9, w10, w11) = (w[8][k], w[9][k], w[10][k], w[11][k]);
                let mut wf2 = (1.0 - w4) * (1.0 - w1) * (1.0 - w8)
                    + (1.0 - w5) * w0 * (1.0 - w9)
                    + (1.0 - w6) * w3 * w9
                    + (1.0 - w7) * (1.0 - w3) * w8;
                let mut wf3 = w4 * (1.0 - w1) * (1.0 - w11)
                    + w5 * w1 * (1.0 - w10)
                    + w6 * w2 * w10
                    + w7 * (1.0 - w2) * w11;
                let sum = wf2 + wf3;
                wf2 /= sum;
                wf3 /= sum;
                let v = pl(i, j, k);
                points[v] += (fp[2][fpl(2, i, k)] - points[pl(i, 0, k)]) * wf2
                    + (fp[3][fpl(3, i, k)] - points[pl(i, nj, k)]) * wf3;
            }
        }
    }

    // k-faces
    for kk in 0..=nk {
        let k = (kk + 1) % (nk + 1);
        let (w8, w9, w10, w11) = (w[8][k], w[9][k], w[10][k], w[11][k]);
        for i in 0..=ni {
            let (w0, w1, w2, w3) = (w[0][i], w[1][i], w[2][i], w[3][i]);
            for j in 0..=nj {
                if flat(i, j, k) {
                    continue;
                }
                let (w4, w5, w6, w7) = (w[4][j], w[5][j], w[6][j], w[7][j]);
                let mut wf4 = (1.0 - w8) * (1.0 - w0) * (1.0 - w4)
                    + (1.0 - w9) * w0 * (1.0 - w5)
                    + (1.0 - w10) * w1 * w5
                    + (1.0 - w11) * (1.0 - w1) * w4;
                let mut wf5 = w8 * (1.0 - w3) * (1.0 - w7)
                    + w9 * w3 * (1.0 - w6)
                    + w10 * w2 * w6
                    + w11 * (1.0 - w2) * w7;
                let sum = wf4 + wf5;
                wf4 /= sum;
                wf5 /= sum;
                let v = pl(i, j, k);
                points[v] += (fp[4][fpl(4, i, j)] - points[pl(i, j, 0)]) * wf4
                    + (fp[5][fpl(5, i, j)] - points[pl(i, j, nk)]) * wf5;
            }
        }
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────────────────
//  geometry dictionary
// ───────────────────────────────────────────────────────────────────────────

/// One entry of a parsed dictionary body.
#[derive(Debug, Clone, PartialEq)]
pub enum DictEntry {
    /// `key tokens… ;`
    Tokens(Vec<String>),
    /// `key { … }`
    Dict(Vec<(String, DictEntry)>),
}

/// Read `{ key value; key { … } … }` starting at the `{` at `*pos`.
///
/// # Errors
/// [`MeshError::DictParse`] on an unterminated dictionary.
pub fn read_dict_body(tokens: &[String], pos: &mut usize) -> Result<Vec<(String, DictEntry)>, MeshError> {
    let bad = || MeshError::DictParse("unterminated dictionary in `geometry`".into());
    if tokens.get(*pos).map(String::as_str) != Some("{") {
        return Err(MeshError::DictParse("expected `{` in `geometry`".into()));
    }
    *pos += 1;
    let mut out = Vec::new();
    loop {
        let key = tokens.get(*pos).ok_or_else(bad)?.clone();
        *pos += 1;
        if key == "}" {
            return Ok(out);
        }
        if tokens.get(*pos).map(String::as_str) == Some("{") {
            let sub = read_dict_body(tokens, pos)?;
            out.push((key, DictEntry::Dict(sub)));
        } else {
            let mut v = Vec::new();
            loop {
                let t = tokens.get(*pos).ok_or_else(bad)?.clone();
                *pos += 1;
                if t == ";" {
                    break;
                }
                v.push(t);
            }
            out.push((key, DictEntry::Tokens(v)));
        }
    }
}

fn lookup<'a>(d: &'a [(String, DictEntry)], keys: &[&str]) -> Option<&'a DictEntry> {
    d.iter().find(|(k, _)| keys.contains(&k.as_str())).map(|(_, v)| v)
}

fn tokens_of<'a>(d: &'a [(String, DictEntry)], keys: &[&str], surf: &str) -> Result<&'a [String], MeshError> {
    match lookup(d, keys) {
        Some(DictEntry::Tokens(t)) => Ok(t),
        _ => Err(MeshError::DictParse(format!(
            "geometry `{surf}`: missing `{}`",
            keys[0]
        ))),
    }
}

fn scalar_of(t: &[String], surf: &str, key: &str) -> Result<f64, MeshError> {
    t.first()
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or_else(|| MeshError::DictParse(format!("geometry `{surf}`: bad `{key}`")))
}

fn vector_of(t: &[String], surf: &str, key: &str) -> Result<Vector3, MeshError> {
    let nums: Vec<f64> = t
        .iter()
        .filter(|s| *s != "(" && *s != ")")
        .map(|s| s.parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| MeshError::DictParse(format!("geometry `{surf}`: bad `{key}`")))?;
    if nums.len() != 3 {
        return Err(MeshError::DictParse(format!(
            "geometry `{surf}`: `{key}` needs three components"
        )));
    }
    Ok(Vector3::new(nums[0], nums[1], nums[2]))
}

impl Geometry {
    /// Build the surface list from the body of the `geometry { … }` entry.
    ///
    /// # Errors
    /// [`MeshError::NotImplemented`] for a surface `type` that is not ported;
    /// [`MeshError::DictParse`] for a missing or malformed key.
    pub fn from_entries(
        entries: &[(String, DictEntry)],
        flavour: ProjectionFlavour,
    ) -> Result<Self, MeshError> {
        let mut surfaces = Vec::new();
        for (name, entry) in entries {
            let DictEntry::Dict(d) = entry else {
                return Err(MeshError::DictParse(format!(
                    "geometry entry `{name}` is not a dictionary"
                )));
            };
            let ty = tokens_of(d, &["type"], name)?
                .first()
                .cloned()
                .unwrap_or_default();
            let surface = match ty.as_str() {
                "sphere" | "searchableSphere" => SearchableSurface::Sphere {
                    centre: vector_of(tokens_of(d, &["centre", "origin"], name)?, name, "centre")?,
                    radius: scalar_of(tokens_of(d, &["radius"], name)?, name, "radius")?,
                },
                "cylinder" | "searchableCylinder" => SearchableSurface::Cylinder {
                    point1: vector_of(tokens_of(d, &["point1"], name)?, name, "point1")?,
                    point2: vector_of(tokens_of(d, &["point2"], name)?, name, "point2")?,
                    radius: scalar_of(tokens_of(d, &["radius"], name)?, name, "radius")?,
                },
                "plane" | "searchablePlane" => {
                    // `planeType pointAndNormal` with an optional sub-dictionary,
                    // or the keys directly (dev `plane(dictionary)`).
                    let sub: &[(String, DictEntry)] = match lookup(d, &["pointAndNormalDict"]) {
                        Some(DictEntry::Dict(s)) => s,
                        _ => d,
                    };
                    if lookup(sub, &["a", "b", "c", "d", "point1", "point2", "point3"]).is_some() {
                        return Err(MeshError::NotImplemented(format!(
                            "geometry `{name}`: plane given by equation or three points \
                             (only pointAndNormal is ported; gh:#1462)"
                        )));
                    }
                    SearchableSurface::plane(
                        vector_of(
                            tokens_of(sub, &["point", "basePoint"], name)?,
                            name,
                            "point",
                        )?,
                        vector_of(
                            tokens_of(sub, &["normal", "normalVector"], name)?,
                            name,
                            "normal",
                        )?,
                    )
                }
                other => {
                    return Err(MeshError::NotImplemented(format!(
                        "geometry `{name}`: searchable surface type `{other}` \
                         (ported: sphere, cylinder, plane; gh:#1462)"
                    )))
                }
            };
            surfaces.push((name.clone(), surface));
        }
        Ok(Geometry { surfaces, flavour })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Mat, v: Vector3) -> Vector3 {
        mat_vec(m, v)
    }

    /// `rotationTensor(n1, n2)` maps n1 to n2 in the three branches
    /// (general, contradirectional, codirectional) and is a proper rotation.
    #[test]
    fn rotation_tensor_branches() {
        let x = Vector3::new(1.0, 0.0, 0.0);
        let y = Vector3::new(0.0, 1.0, 0.0);
        for (a, b) in [(x, y), (x, -x), (x, x), (y, -y)] {
            let r = rotation_tensor(a, b);
            let m = apply(&r, a);
            assert!((m - b).mag() < 1e-15, "{a:?} -> {b:?}: {m:?}");
            // orthonormal: R R^T = I
            let rt: Mat = std::array::from_fn(|i| std::array::from_fn(|j| r[j][i]));
            let id = mat_mul(&r, &rt);
            for i in 0..3 {
                for j in 0..3 {
                    let want = if i == j { 1.0 } else { 0.0 };
                    assert!((id[i][j] - want).abs() < 1e-15);
                }
            }
        }
    }

    /// `Ra(a, omega)` is the right-handed rotation about `a`, and scaling a
    /// matrix scales its action.
    #[test]
    fn rotation_about_axis_and_scale() {
        let z = Vector3::new(0.0, 0.0, 1.0);
        let r = rotation_about(z, std::f64::consts::FRAC_PI_2);
        let v = apply(&r, Vector3::new(1.0, 0.0, 0.0));
        assert!((v - Vector3::new(0.0, 1.0, 0.0)).mag() < 1e-15);
        let v = apply(&mat_scale(&r, 2.0), Vector3::new(1.0, 0.0, 0.0));
        assert!((v - Vector3::new(0.0, 2.0, 0.0)).mag() < 1e-15);
    }

    /// `plane::planeIntersect`: the line of two perpendicular planes, with the
    /// zeroed component chosen from the largest component of the direction.
    #[test]
    fn plane_intersection_line() {
        let z = Vector3::new(0.0, 0.0, 1.0);
        let x = Vector3::new(1.0, 0.0, 0.0);
        let y = Vector3::new(0.0, 1.0, 0.0);
        let r = plane_intersect(Vector3::new(0.0, 0.0, 2.0), z, Vector3::new(3.0, 0.0, 0.0), x);
        assert!((r.dir - Vector3::new(0.0, 1.0, 0.0)).mag() < 1e-15);
        assert!((r.ref_point - Vector3::new(3.0, 0.0, 2.0)).mag() < 1e-15);
        let r = plane_intersect(Vector3::new(0.0, 5.0, 0.0), y, Vector3::new(1.0, 0.0, 0.0), x);
        assert!((r.ref_point - Vector3::new(1.0, 5.0, 0.0)).mag() < 1e-15);
        // both line-projection flavours land on the line from an off-line point
        let near = Vector3::new(3.5, 0.5, 2.5);
        let line = plane_intersect(Vector3::new(0.0, 0.0, 2.0), z, Vector3::new(3.0, 0.0, 0.0), x);
        for d in [onto_line(&line, near), onto_line_esi(&line, near)] {
            let p = near + d;
            assert!((p.x - 3.0).abs() < 1e-15 && (p.z - 2.0).abs() < 1e-15);
        }
    }

    /// `linearInterpolationWeights`: end clamps and the interior weights, both
    /// flavours.
    #[test]
    fn linear_weights_both_flavours() {
        let s = [0.0, 0.25, 1.0];
        for fl in [ProjectionFlavour::Dev, ProjectionFlavour::Esi] {
            let mut w = LinearWeights::new(&s, fl);
            let (iw, n) = w.value_weights(-1.0);
            assert_eq!((iw[0].0, n), (0, 1));
            let (iw, n) = w.value_weights(0.625);
            assert_eq!(n, 2);
            assert_eq!((iw[0].0, iw[1].0), (1, 2));
            assert!((iw[0].1 - 0.5).abs() < 1e-15 && (iw[1].1 - 0.5).abs() < 1e-15);
            let (iw, n) = w.value_weights(2.0);
            assert_eq!((iw[0].0, n), (2, 1));
            // cached slot reused
            let (iw, _) = w.value_weights(0.5);
            assert_eq!(iw[0].0, 1);
        }
    }

    /// An empty surface list is an error; a list of known names resolves;
    /// `sweep_transpose` follows the first anchor edge found in the loop.
    #[test]
    fn surface_ids_errors_and_sweep_transpose() {
        let g = Geometry {
            surfaces: vec![(
                "a".into(),
                SearchableSurface::plane(Vector3::ZERO, Vector3::new(0.0, 0.0, 2.0)),
            )],
            flavour: ProjectionFlavour::Dev,
        };
        assert!(g.surface_ids(&[]).is_err());
        assert!(g.surface_ids(&["b".to_string()]).is_err());
        assert_eq!(g.surface_ids(&["a".to_string()]).unwrap(), vec![0]);
        assert!(!sweep_transpose(&[0, 1, 2, 3, 4, 5, 6, 7], 2, &[0, 1, 5, 4]));
        assert!(sweep_transpose(&[0, 1, 2, 3, 4, 5, 6, 7], 2, &[1, 5, 4, 0]));
        assert!(!sweep_transpose(&[0, 1, 2, 3, 4, 5, 6, 7], 2, &[9, 8, 7, 6]));
    }
}
