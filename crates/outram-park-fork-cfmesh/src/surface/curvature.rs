// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/triSurfaceTools/triSurfaceCurvatureEstimator/
//     triSurfaceCurvatureEstimator{.C,.H,Calculate.C}
//   meshLibrary/utilities/helperClasses/geometry/quadricFitting/
//     quadricFitting{.H,I.H} (the constructor with a given normal)
//   OpenFOAM simpleMatrix::LUsolve / LUDecompose / LUBacksubstitute (the
//     5 x 5 solve quadricFitting calls), Crout's method with implicit partial
//     pivoting.
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd.; OpenFOAM (C) OpenFOAM Foundation /
// OpenCFD Ltd. Licence: GPL-3.0-or-later, conveyed here under GPL-3.0-only.
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

//! `triSurfaceCurvatureEstimator` and `quadricFitting` (gh:#1278,
//! `docs/upstream_coverage.md` §4 item 24). `meshOctreeAutomaticRefinement`,
//! the consumer, is **not** ported yet.
//!
//! At each point and for each patch meeting there, a quadric
//! `z = a x² + b y² + c xy + d x + e y` is fitted (least squares) to the
//! point's neighbours in that patch, in a frame whose `z` is the patch
//! normal; the normal is corrected from `d`, `e` and the fit repeated (10
//! times). The principal curvatures are `a + b ± √((a − b)² + c²)`; the
//! minimum and maximum are then smoothed by two Jacobi passes of
//! `½ (own + neighbour mean)`. The sign follows the normal: on an outward
//! wound sphere both curvatures are `−1/R`.
//!
//! Port notes:
//! - Upstream iterates the per-point patch maps in hash order; here patches
//!   are ascending and the neighbour points ascending (this only changes
//!   which neighbour fixes the in-plane `x` axis, so the curvature vectors,
//!   not the curvatures).
//! - Upstream sums `labelledTri::normal` (not re-checked whether that is the
//!   area vector or the unit normal in the OpenFOAM it builds against; the
//!   port uses the area vector) once per other point of each triangle.
//! - The smoothing divides by the neighbour count with no guard; a patch
//!   point with no neighbour would become `NaN` upstream. The port keeps
//!   the unsmoothed value there.
//! - `calculateEdgeCurvature` uses the stored direction of each surface
//!   edge (lower point to higher), so at a point where its two feature
//!   edges both start or both end the angle is measured between the wrong
//!   directions (`π − θ` instead of `θ`). Ported as written; the test
//!   records it on the pipe rims.

use crate::math::Vec3;
use crate::mesh_octree::{TriSurf, SMALL, VSMALL};
use petir::mathf::RealMath;
use std::collections::BTreeMap;

/// `simpleMatrix<scalar>::LUsolve` for a small dense system: Crout LU with
/// implicit (row-scaled) partial pivoting, then back substitution. A
/// singular matrix gives non-finite entries, as upstream.
pub fn lu_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    let mut vv = vec![0.0; n];
    for i in 0..n {
        let big = a[i].iter().fold(0.0f64, |m, &x| m.max(x.abs()));
        vv[i] = 1.0 / big;
    }
    let mut piv = vec![0usize; n];
    for j in 0..n {
        for i in 0..j {
            let mut s = a[i][j];
            for k in 0..i {
                s -= a[i][k] * a[k][j];
            }
            a[i][j] = s;
        }
        let mut big = 0.0;
        let mut imax = j;
        for i in j..n {
            let mut s = a[i][j];
            for k in 0..j {
                s -= a[i][k] * a[k][j];
            }
            a[i][j] = s;
            let d = vv[i] * s.abs();
            if d >= big {
                big = d;
                imax = i;
            }
        }
        if j != imax {
            a.swap(imax, j);
            vv[imax] = vv[j];
        }
        piv[j] = imax;
        if a[j][j] == 0.0 {
            a[j][j] = SMALL;
        }
        if j != n - 1 {
            let r = 1.0 / a[j][j];
            for row in a.iter_mut().skip(j + 1) {
                row[j] *= r;
            }
        }
    }
    // Back substitution (forward with the row swaps, then backward).
    let mut ii: Option<usize> = None;
    for i in 0..n {
        let ip = piv[i];
        let mut s = b[ip];
        b[ip] = b[i];
        if let Some(i0) = ii {
            for j in i0..i {
                s -= a[i][j] * b[j];
            }
        } else if s != 0.0 {
            ii = Some(i);
        }
        b[i] = s;
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= a[i][j] * b[j];
        }
        b[i] = s / a[i][i];
    }
    b
}

/// `quadricFitting` (the constructor with a given normal).
#[derive(Debug, Clone, PartialEq)]
pub struct QuadricFitting {
    /// `origin_`.
    pub origin: Vec3,
    /// `normal_` after the corrections.
    pub normal: Vec3,
    /// `vecX_`.
    pub vec_x: Vec3,
    /// `vecY_`.
    pub vec_y: Vec3,
    /// `coefficients_`: `a, b, c, d, e`.
    pub coefficients: [f64; 5],
}

impl QuadricFitting {
    /// Fit at `origin` with the initial `normal` to `others`. Upstream exits
    /// fatally for a zero normal; the port returns zero coefficients.
    pub fn new(origin: Vec3, normal: Vec3, others: &[Vec3]) -> Self {
        let m = normal.length();
        let mut q = QuadricFitting {
            origin,
            normal: if m * m < VSMALL { Vec3::ZERO } else { normal.div(m) },
            vec_x: Vec3::ZERO,
            vec_y: Vec3::ZERO,
            coefficients: [0.0; 5],
        };
        // calculateBestFit.
        for _ in 0..10 {
            let tp = q.coordinate_system(others);
            q.quadric_coeffs(&tp);
            let c = q.coefficients;
            if c[3].abs() > SMALL || c[4].abs() > SMALL {
                let d = (1.0 + c[3] * c[3] + c[4] * c[4]).sqrt();
                // `normal_/d - c3*vecX_/d - c4*vecY_/d`: ((c3*vecX)/d), each component
                q.normal = q.normal.div(d).sub(q.vec_x.scale(c[3]).div(d)).sub(q.vec_y.scale(c[4]).div(d));
            } else {
                break;
            }
        }
        q
    }

    /// `calculateCoordinateSystem`: the frame and the transformed points.
    /// Upstream loops past the end if every point projects onto the origin;
    /// the port then leaves the axes zero.
    fn coordinate_system(&mut self, others: &[Vec3]) -> Vec<Vec3> {
        if self.normal.length() < VSMALL {
            self.vec_x = Vec3::ZERO;
            self.vec_y = Vec3::ZERO;
            return vec![Vec3::ZERO; others.len()];
        }
        // `plane(origin_, normal_)`: `unitVector_ = normal; unitVector_ /= mag`
        let n = self.normal.div(self.normal.length());
        for &p in others {
            let vp = p.sub(n.scale(p.sub(self.origin).dot(n)));
            let vx = vp.sub(self.origin);
            if vx.dot(vx) < VSMALL {
                continue;
            }
            self.vec_x = vx.div(vx.length());
            self.vec_y = self.normal.cross(self.vec_x);
            break;
        }
        others
            .iter()
            .map(|&p| {
                let d = p.sub(self.origin);
                Vec3::new(d.dot(self.vec_x), d.dot(self.vec_y), d.dot(self.normal))
            })
            .collect()
    }

    /// `calculateQuadricCoeffs`: the normal equations, `SMALL` on a zero
    /// diagonal, `LUsolve`.
    fn quadric_coeffs(&mut self, tp: &[Vec3]) {
        if self.normal.length() < VSMALL {
            self.coefficients = [0.0; 5];
            return;
        }
        let mut m = vec![vec![0.0; 5]; 5];
        let mut src = vec![0.0; 5];
        for p in tp {
            let h = [p.x * p.x, p.y * p.y, p.x * p.y, p.x, p.y];
            for r in 0..5 {
                for c in r..5 {
                    m[r][c] += h[r] * h[c];
                }
                src[r] += h[r] * p.z;
            }
        }
        for r in 1..5 {
            for c in 0..r {
                m[r][c] = m[c][r];
            }
            if m[r][r].abs() < SMALL {
                m[r][r] = SMALL;
            }
        }
        let x = lu_solve(m, src);
        self.coefficients = [x[0], x[1], x[2], x[3], x[4]];
    }

    /// `minCurvature()`.
    pub fn min_curvature(&self) -> f64 {
        let c = self.coefficients;
        c[0] + c[1] - ((c[0] - c[1]) * (c[0] - c[1]) + c[2] * c[2]).sqrt()
    }

    /// `maxCurvature()`.
    pub fn max_curvature(&self) -> f64 {
        let c = self.coefficients;
        c[0] + c[1] + ((c[0] - c[1]) * (c[0] - c[1]) + c[2] * c[2]).sqrt()
    }

    /// `theta` of the curvature vectors (`stabilise(a − b, VSMALL)`).
    fn theta(&self) -> f64 {
        let c = self.coefficients;
        let d = c[0] - c[1];
        let s = if d >= 0.0 { d + VSMALL } else { d - VSMALL };
        0.5 * (c[2] / s).r_atan()
    }

    /// `minCurvatureVector()`.
    pub fn min_curvature_vector(&self) -> Vec3 {
        let t = self.theta();
        self.vec_x.scale(t.r_cos()).add(self.vec_y.scale(t.r_sin()))
    }

    /// `maxCurvatureVector()` (as upstream: `sin θ x + cos θ y`).
    pub fn max_curvature_vector(&self) -> Vec3 {
        let t = self.theta();
        self.vec_x.scale(t.r_sin()).add(self.vec_y.scale(t.r_cos()))
    }
}

/// OpenFOAM's `labelHashSet`, as far as its **iteration order** goes (gh:#1385
/// round 3): `HashTable::setEntry` (capacity 128 on the first insert,
/// `Hash<label>` is the label itself and the bucket is `label & (capacity - 1)`,
/// a new node goes to the head of its bucket's chain, and `0.8 * capacity <
/// size` doubles the capacity, re-chaining bucket by bucket with each node
/// moved to the head of its new bucket). Iteration is bucket 0, 1, ... with
/// each chain from its head. `quadricFitting` sums over `otherLabels` in this
/// order, and with five to seven points the normal equations are nearly
/// singular, so the order is not a round-off matter. Reference:
/// OpenFOAM-v2506 `HashTable.C`, `HashTableI.H`, `Hash.H`.
#[derive(Debug, Clone, Default)]
struct FoamLabelHashSet {
    /// Buckets, each a chain with its head first; empty until the first insert.
    table: Vec<Vec<usize>>,
    size: usize,
}

impl FoamLabelHashSet {
    fn insert(&mut self, key: usize) {
        if self.table.is_empty() {
            self.table = vec![Vec::new(); 128];
        }
        let cap = self.table.len();
        let bucket = &mut self.table[key & (cap - 1)];
        if bucket.contains(&key) {
            return;
        }
        bucket.insert(0, key);
        self.size += 1;
        if 0.8 * (cap as f64) < self.size as f64 {
            self.set_capacity(2 * cap);
        }
    }

    fn set_capacity(&mut self, new_cap: usize) {
        let mut table = vec![Vec::new(); new_cap];
        for chain in &self.table {
            for &k in chain {
                table[k & (new_cap - 1)].insert(0, k);
            }
        }
        self.table = table;
    }

    /// The keys in iteration order.
    fn order(&self) -> Vec<usize> {
        self.table.iter().flatten().copied().collect()
    }
}

/// `triSurfaceCurvatureEstimator`. Per point, the values are listed per
/// patch at that point (`point_patches`, ascending).
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurfaceCurvatureEstimator {
    /// `edgePointCurvature_`: curvature of the feature-edge line at points
    /// with exactly two patch-boundary edges, 0 elsewhere.
    pub edge_point_curvature: Vec<f64>,
    /// The patches at each point (`pointPatches`).
    pub point_patches: Vec<Vec<usize>>,
    /// `patchPositions_`: for facet `t` and corner `i`, the index of `t`'s
    /// patch in `point_patches[tri[i]]`.
    pub patch_positions: Vec<[usize; 3]>,
    /// `gaussianCurvature_` (after smoothing: `min × max`).
    pub gaussian: Vec<Vec<f64>>,
    /// `meanCurvature_` (after smoothing: `½ (min + max)`).
    pub mean: Vec<Vec<f64>>,
    /// `maxCurvature_` (smoothed).
    pub max: Vec<Vec<f64>>,
    /// `minCurvature_` (smoothed).
    pub min: Vec<Vec<f64>>,
    /// `maxCurvatureVector_` (not smoothed).
    pub max_vector: Vec<Vec<Vec3>>,
    /// `minCurvatureVector_` (not smoothed).
    pub min_vector: Vec<Vec<Vec3>>,
}

impl TriSurfaceCurvatureEstimator {
    /// The constructor: `calculateEdgeCurvature`, `calculateSurfaceCurvatures`.
    pub fn new(surf: &TriSurf) -> Self {
        let edge_point_curvature = edge_curvature(surf);
        let np = surf.n_points();
        let mut point_patches = vec![Vec::new(); np];
        let mut patch_positions = vec![[0usize; 3]; surf.size()];
        let (mut gaussian, mut mean, mut max, mut min) = (vec![Vec::new(); np], vec![Vec::new(); np], vec![Vec::new(); np], vec![Vec::new(); np]);
        let (mut max_vector, mut min_vector) = (vec![Vec::new(); np], vec![Vec::new(); np]);
        for p in 0..np {
            let mut region_tris: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            let mut others: BTreeMap<usize, FoamLabelHashSet> = BTreeMap::new();
            let mut normals: BTreeMap<usize, Vec3> = BTreeMap::new();
            for &t in &surf.point_facets[p] {
                let r = surf.regions[t];
                region_tris.entry(r).or_default().push(t);
                let n = normals.entry(r).or_insert(Vec3::ZERO);
                let o = others.entry(r).or_default();
                for &q in &surf.triangles[t] {
                    if q == p {
                        continue;
                    }
                    o.insert(q);
                    *n = n.add(surf.facet_area_vector(t));
                }
            }
            // Fewer than six neighbours: add the next ring (in the patch),
            // through a second hash set, in hash order, as upstream.
            for (r, set) in others.iter_mut() {
                if set.size > 5 {
                    continue;
                }
                let mut additional = FoamLabelHashSet::default();
                for q in set.order() {
                    for &t in &surf.point_facets[q] {
                        if surf.regions[t] != *r {
                            continue;
                        }
                        for &v in &surf.triangles[t] {
                            additional.insert(v);
                        }
                    }
                }
                for a in additional.order() {
                    set.insert(a);
                }
            }
            for n in normals.values_mut() {
                // `nit() /= (mag(nit()) + VSMALL)`
                *n = n.div(n.length() + VSMALL);
            }
            for (r, set) in &others {
                point_patches[p].push(*r);
                for &t in &region_tris[r] {
                    if let Some(pos) = surf.triangles[t].iter().position(|&q| q == p) {
                        patch_positions[t][pos] = gaussian[p].len();
                    }
                }
                let op: Vec<Vec3> = set.order().into_iter().map(|q| surf.points[q]).collect();
                let q = QuadricFitting::new(surf.points[p], normals[r], &op);
                let (kmin, kmax) = (q.min_curvature(), q.max_curvature());
                gaussian[p].push(kmin * kmax);
                mean[p].push(0.5 * (kmin + kmax));
                max[p].push(kmax);
                min[p].push(kmin);
                max_vector[p].push(q.max_curvature_vector());
                min_vector[p].push(q.min_curvature_vector());
            }
        }
        // Two Jacobi passes of the weighted Laplace smoothing.
        for _ in 0..2 {
            let mut smin = min.clone();
            let mut smax = max.clone();
            for p in 0..np {
                let mut nei: BTreeMap<usize, Vec<usize>> = point_patches[p].iter().map(|&r| (r, Vec::new())).collect();
                for &t in &surf.point_facets[p] {
                    let l = nei.entry(surf.regions[t]).or_default();
                    for &q in &surf.triangles[t] {
                        if q != p && !l.contains(&q) {
                            l.push(q);
                        }
                    }
                }
                for (i, r) in point_patches[p].iter().enumerate() {
                    let l = &nei[r];
                    if l.is_empty() {
                        continue;
                    }
                    let (mut a, mut b) = (0.0, 0.0);
                    for &q in l {
                        let pos = point_patches[q].iter().position(|x| x == r).unwrap_or(0);
                        a += min[q][pos];
                        b += max[q][pos];
                    }
                    a /= l.len() as f64;
                    b /= l.len() as f64;
                    smin[p][i] = 0.5 * (a + min[p][i]);
                    smax[p][i] = 0.5 * (b + max[p][i]);
                }
            }
            min = smin;
            max = smax;
        }
        for p in 0..np {
            for i in 0..min[p].len() {
                gaussian[p][i] = min[p][i] * max[p][i];
                mean[p][i] = 0.5 * (min[p][i] + max[p][i]);
            }
        }
        let est = TriSurfaceCurvatureEstimator { edge_point_curvature, point_patches, patch_positions, gaussian, mean, max, min, max_vector, min_vector };
        if std::env::var("GH1385_TRACE").is_ok() {
            // gh:#1385: the instrumented upstream prints the same (`ECURV`, `MCURV`)
            for (p, k) in est.edge_point_curvature.iter().enumerate() {
                eprintln!("ECURV {p} {k:.17e}");
            }
            for t in 0..surf.size() {
                eprintln!("MCURV {t} {:.17e}", est.mean_curvature_at_triangle(surf, t));
            }
        }
        est
    }

    /// `curvatureAtEdge`: the mean of its two points' edge curvature.
    pub fn curvature_at_edge(&self, surf: &TriSurf, e: usize) -> f64 {
        let [a, b] = surf.edges[e];
        0.5 * (self.edge_point_curvature[a] + self.edge_point_curvature[b])
    }

    fn at_triangle(&self, surf: &TriSurf, field: &[Vec<f64>], t: usize) -> f64 {
        let tr = surf.triangles[t];
        (0..3).map(|i| field[tr[i]][self.patch_positions[t][i]]).sum::<f64>() / 3.0
    }

    /// `meanCurvatureAtTriangle`.
    pub fn mean_curvature_at_triangle(&self, surf: &TriSurf, t: usize) -> f64 {
        self.at_triangle(surf, &self.mean, t)
    }

    /// `gaussianCurvatureAtTriangle`.
    pub fn gaussian_curvature_at_triangle(&self, surf: &TriSurf, t: usize) -> f64 {
        self.at_triangle(surf, &self.gaussian, t)
    }

    /// `maxCurvatureAtTriangle`.
    pub fn max_curvature_at_triangle(&self, surf: &TriSurf, t: usize) -> f64 {
        self.at_triangle(surf, &self.max, t)
    }

    /// `minCurvatureAtTriangle`.
    pub fn min_curvature_at_triangle(&self, surf: &TriSurf, t: usize) -> f64 {
        self.at_triangle(surf, &self.min, t)
    }
}

/// `calculateEdgeCurvature`: at a point with exactly two patch-boundary
/// edges (two facets, different regions), `acos(ê1·ê2) / ½(|e1| + |e2|)`
/// with the edges' stored directions.
fn edge_curvature(surf: &TriSurf) -> Vec<f64> {
    let feature: Vec<bool> = surf.edge_facets.iter().map(|ef| ef.len() == 2 && surf.regions[ef[0]] != surf.regions[ef[1]]).collect();
    let mut out = vec![0.0; surf.n_points()];
    for (p, pe) in surf.point_edges.iter().enumerate() {
        let f: Vec<usize> = pe.iter().copied().filter(|&e| feature[e]).collect();
        if f.len() != 2 {
            continue;
        }
        let vec = |e: usize| surf.points[surf.edges[e][1]].sub(surf.points[surf.edges[e][0]]);
        let (e1, e2) = (vec(f[0]), vec(f[1]));
        let (d1, d2) = (e1.length() + VSMALL, e2.length() + VSMALL);
        let cs = (e1.div(d1).dot(e2.div(d2))).clamp(-1.0, 1.0);
        out[p] = (cs.r_acos() / (0.5 * (d1 + d2 + VSMALL))).abs();
    }
    out
}
