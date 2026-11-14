// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI v2506
// applications/utilities/preProcessing/viewFactorsGen/{viewFactorsGen.C, shootRays.H,
// searchingEngine.H}
// Copyright (C) 2011-2016 OpenFOAM Foundation
// Copyright (C) 2016-2024 OpenCFD Ltd.
//! `viewFactorsGen` (ESI v2506): view-factor generator, serial, no face agglomeration
//! (gh:#1540, under gh:#1529, #1336).
//!
//! # What is ported
//!
//! | Upstream (`viewFactorsGen.C` unless noted) | Here |
//! |---|---|
//! | coarse `Cf`/`Sf` of an un-agglomerated patch (`singleCellFvMesh` face, nearest fine centre/vertex snap) | the fine face centre and area vector, [`crate::io::poly_mesh::face_centre_and_area`] |
//! | candidate rays `(d & fA) < 0 && (d & remA) > 0`, `start = fc + 0.001 d`, `end = fc + 0.999 d` (`shootRays.H`) | [`generate_view_factors`] |
//! | visibility by `distributedTriSurfaceMesh::findLine` (serial: `indexedOctree::findLine`) and the self-hit continuation loop (`shootRays.H`) | ~~brute-force nearest segment/triangle hit~~ **CORRECTED 2026-11-14 (gh:#1564)** the octree itself, [`crate::esi_octree::Octree::find_line`], same loop (see "Visibility") |
//! | `calculateViewFactorFij2AI`, far test, `GaussQuad`, the Gauss tables, `GaussQuadTol`/`distTol`/`alpha` | private functions, same operation order |
//! | `F` (`scalarListIOList`), `globalFaceFaces` (`labelListIOList`), `mapDist` (`IOmapDistribute`, one processor) | [`write_scalar_list_list_file`], [`write_label_list_list_file`], [`write_map_dist_file`] |
//!
//! NOT ported (deferred, gh:#1540 ledger): `faceAgglomerate` / `finalAgglom` /
//! `singleCellFvMesh` / `pairPatchAgglomeration` (coarse faces of several fine faces),
//! the 2-D (`nSolutionD == 2`) branch, parallel `mapDistribute`, `dumpRays`,
//! `writeViewFactorMatrix` (the `viewFactorField`), and the CGAL ray engine (this is the
//! `-DNO_CGAL` octree route, which is what the ESI reference was built with).
//!
//! # Method
//!
//! For fine face `i` with centre `Ci` and area vector `Si`, and each visible face `j`:
//! if ANY (fine i, fine j) pair has `|Ci - Cj| / ((a_i + a_j)/2) > distTol`
//! (`a = sqrt(|S|/pi)`) the pair is "far" and `F_ij = sum_{i,j} cos_i cos_j |S_i||S_j| /
//! (pi r^2) / |A_i|` (two area integrals, 2AI); otherwise it is "near" and the double
//! contour integral (2LI) `F_ij = (1/(4 pi |A_i|)) sum_edges sum_edges cos(si,sj) *
//! int int ln(r.r)` is evaluated with Gauss-Legendre of order 1..5 until the relative change
//! is below `GaussQuadTol` (default **0.01** in the code; the header comment of upstream
//! says 0.1). Edges that share a point use order 0 with `r = alpha |si| di`.
//!
//! # Visibility
//!
//! ~~A candidate ray is a hit if it intersects any wall triangle (nearest hit, Moller-Trumbore,
//! no tolerance) ... brute force over all triangles ... the two agree whenever no ray grazes a
//! triangle edge.~~ **CORRECTED 2026-11-14 (gh:#1564)**: the brute force did NOT agree with
//! upstream on rays lying in the plane of a wall triangle (coplanar faces, the non-convex L case,
//! the thin graded box): `indexedOctree::findLine` does not test the ray's own (start, end) against
//! each triangle. It walks the octree leaves in ray order and, in each leaf, calls the triangle
//! test with the point where the ray ENTERED the leaf (pushed onto the leaf face) and the
//! nearest hit so far as end point, accepting a hit only if it lies inside that leaf. For an
//! in-plane ray `det`, `u`, `v`, `t` are ~1e-17 noise that depends on those arguments. The visibility
//! therefore runs on a port of the octree (`crate::esi_octree`, built as `triSurfaceSearch::tree()`:
//! depth 10, leaf size 10, duplicity 3, bounds inflated by `Random(65431)`), with the unchanged
//! upstream triangle test (`triangle::intersection(HALF_RAY)`, `planarTol = 10*SMALL`, `|det| <
//! ROOTVSMALL`, `distance() <= 1`). A hit on a triangle of the ray's own face is continued from the
//! hit point (`+0.01 d`, `+1.01 d`, at most 10 passes), a hit on the target face counts as visible,
//! anything else blocks the ray, as in `shootRays.H`. Triangles are `face::triangles` (a quad
//! starts at its most concave angle, as upstream).
//!
//! # Transcendentals
//!
//! `ln` is [`petir::mathf::RealMath::r_ln`] (one implementation on every target, workspace
//! gate `no_platform_libm`); upstream calls glibc `log`. See the V&V record for how many
//! `F` entries differ in the last bit, if any.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

use crate::esi_octree::{LineShapes, Octree, OctreeShapes, TreeBox, V};
use crate::io::poly_mesh::face_centre_and_area;
use crate::primitives::{Vector3, GREAT, ROOT_VSMALL, SMALL, VSMALL};
use petir::mathf::RealMath;

/// `constant::mathematical::pi` (OpenFOAM: `M_PI`).
const PI: f64 = std::f64::consts::PI;

/// `viewFactorsDict` entries read by `viewFactorsGen`, with upstream's code defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewFactorGenSettings {
    /// `GaussQuadTol`, code default 0.01.
    pub gauss_quad_tol: f64,
    /// `distTol`, code default 8.
    pub dist_tol: f64,
    /// `alpha`, code default 0.21.
    pub alpha: f64,
}

impl Default for ViewFactorGenSettings {
    /// The defaults of `viewFactorsGen.C` (`getOrDefault`), not the header comment's.
    fn default() -> Self {
        Self {
            gauss_quad_tol: 0.01,
            dist_tol: 8.0,
            alpha: 0.21,
        }
    }
}

/// Faces of the `viewFactorWall` patches, patch by patch in patch-index order.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewFactorGenInput {
    /// Mesh points.
    pub points: Vec<Vector3>,
    /// `patch_faces[p][k]` is the vertex loop of face `k` of the `p`-th wall patch.
    pub patch_faces: Vec<Vec<Vec<usize>>>,
}

/// Output of [`generate_view_factors`]: the contents of the `F` and `globalFaceFaces` files.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedViewFactors {
    /// `F`: per face, the view factors to its visible faces.
    pub f: Vec<Vec<f64>>,
    /// `globalFaceFaces`: per face, the (global) indices of its visible faces, ascending.
    pub global_face_faces: Vec<Vec<usize>>,
}

/// Gauss-Legendre points and weights of order 1..5 exactly as written in `viewFactorsGen.C`.
fn gauss_tables() -> ([Vec<f64>; 5], [Vec<f64>; 5]) {
    let p0 = vec![0.0];
    let p1 = vec![1.0 / 3.0_f64.sqrt(), -1.0 / 3.0_f64.sqrt()];
    let p2 = vec![0.0, (3.0_f64 / 5.0).sqrt(), -(3.0_f64 / 5.0).sqrt()];
    let a3 = (3.0_f64 / 7.0 - (2.0 / 7.0) * (6.0_f64 / 5.0).sqrt()).sqrt();
    let b3 = (3.0_f64 / 7.0 + (2.0 / 7.0) * (6.0_f64 / 5.0).sqrt()).sqrt();
    let p3 = vec![a3, -a3, b3, -b3];
    let a4 = (1.0 / 3.0) * (5.0 - 2.0 * (10.0_f64 / 7.0).sqrt()).sqrt();
    let b4 = (1.0 / 3.0) * (5.0 + 2.0 * (10.0_f64 / 7.0).sqrt()).sqrt();
    let p4 = vec![0.0, a4, -a4, b4, -b4];

    let w0 = vec![2.0];
    let w1 = vec![1.0, 1.0];
    let w2 = vec![8.0 / 9.0, 5.0 / 9.0, 5.0 / 9.0];
    let s30 = 30.0_f64.sqrt();
    let w3 = vec![
        (18.0 + s30) / 36.0,
        (18.0 + s30) / 36.0,
        (18.0 - s30) / 36.0,
        (18.0 - s30) / 36.0,
    ];
    let s70 = 70.0_f64.sqrt();
    let w4 = vec![
        128.0 / 225.0,
        (322.0 + 13.0 * s70) / 900.0,
        (322.0 + 13.0 * s70) / 900.0,
        (322.0 - 13.0 * s70) / 900.0,
        (322.0 - 13.0 * s70) / 900.0,
    ];
    ([p0, p1, p2, p3, p4], [w0, w1, w2, w3, w4])
}

/// `Foam::face::centre(points)` (NOT the mesh's `makeFaceCentresAndAreas` centre: the two
/// differ in the last bit). `viewFactorsGen` takes the coarse-face centre of an
/// un-agglomerated face from `uindirectPrimitivePatch::faceCentres()`, which is this one,
/// and the candidate rays start and end on it.
fn face_centre_of(points: &[Vector3], verts: &[usize]) -> Vector3 {
    let n = verts.len();
    if n == 3 {
        return (points[verts[0]] + points[verts[1]] + points[verts[2]]) * (1.0 / 3.0);
    }
    let mut centre_point = Vector3::ZERO;
    for &v in verts {
        centre_point += points[v];
    }
    centre_point /= n as f64;
    let mut sum_a = 0.0;
    let mut sum_ac = Vector3::ZERO;
    for p in 0..n {
        let next = points[verts[(p + 1) % n]];
        let ttc = points[verts[p]] + next + centre_point;
        let ta = (points[verts[p]] - centre_point)
            .cross(next - centre_point)
            .mag();
        sum_a += ta;
        sum_ac += ttc * ta;
    }
    if sum_a > VSMALL {
        sum_ac / (3.0 * sum_a)
    } else {
        centre_point
    }
}

/// `face::areaNormal` for a polygon (central decomposition, centre point last).
fn face_area_normal(points: &[Vector3], verts: &[usize]) -> Vector3 {
    let n = verts.len();
    let mut c = Vector3::ZERO;
    for &v in verts {
        c += points[v];
    }
    c /= n as f64;
    let mut sum = Vector3::ZERO;
    for p in 0..n {
        let a = points[verts[p]];
        let b = points[verts[(p + 1) % n]];
        sum += (b - a).cross(c - a) * 0.5;
    }
    sum
}

/// `face::mostConcaveAngle` of a quadrilateral.
fn most_concave_angle(points: &[Vector3], verts: &[usize]) -> usize {
    let n = verts.len();
    let normal = face_area_normal(points, verts);
    let edges: Vec<Vector3> = (0..n)
        .map(|i| (points[verts[(i + 1) % n]] - points[verts[i]]).normalise(ROOT_VSMALL))
        .collect();
    let mut index = 0;
    let mut max_angle = -GREAT;
    for i in 0..n {
        let left = edges[(i + n - 1) % n];
        let right = edges[i];
        let edge_normal = right.cross(left);
        let edge_cos = left.dot(right).clamp(-1.0, 1.0);
        let edge_angle = edge_cos.r_acos();
        let angle = if edge_normal.dot(normal) > 0.0 {
            PI + edge_angle
        } else {
            PI - edge_angle
        };
        if angle > max_angle {
            max_angle = angle;
            index = i;
        }
    }
    index
}

/// `face::triangles` (`SPLITTRIANGLE`): triangles as vertex-index triples in upstream's
/// order. A quad starts at its most concave angle; a polygon with more than 4 vertices is
/// split as a fan (upstream searches for the best internal diagonal, not ported).
fn face_triangles(points: &[Vector3], verts: &[usize]) -> Vec<[usize; 3]> {
    let n = verts.len();
    if n == 3 {
        vec![[verts[0], verts[1], verts[2]]]
    } else if n == 4 {
        let s = most_concave_angle(points, verts);
        let (a, b, c, d) = (
            verts[s],
            verts[(s + 1) % 4],
            verts[(s + 2) % 4],
            verts[(s + 3) % 4],
        );
        vec![[a, b, c], [c, d, a]]
    } else {
        (1..n - 1)
            .map(|m| [verts[0], verts[m], verts[m + 1]])
            .collect()
    }
}

/// `calculateViewFactorFij2AI`: differential element exchange between two points.
fn view_factor_2ai(i: Vector3, j: Vector3, d_ai: Vector3, d_aj: Vector3) -> f64 {
    let r = i - j;
    let r_mag = r.mag();
    if r_mag > SMALL {
        let d_ai_mag = d_ai.mag();
        let d_aj_mag = d_aj.mag();
        let ni = d_ai / d_ai_mag;
        let nj = d_aj / d_aj_mag;
        let cos_theta_j = nj.dot(r).abs() / r_mag;
        let cos_theta_i = ni.dot(r).abs() / r_mag;
        (cos_theta_i * cos_theta_j * d_aj_mag * d_ai_mag) / ((r_mag * r_mag) * PI)
    } else {
        0.0
    }
}

/// Everything `GaussQuad` takes besides the quadrature tables.
struct EdgePair {
    mag_si: f64,
    mag_sj: f64,
    di: Vector3,
    dj: Vector3,
    ci: Vector3,
    cj: Vector3,
    cosij: f64,
}

/// `GaussQuad`: edge-edge double line integral of `cos ln(r.r)`; `gi` is the order index.
fn gauss_quad(w: &[f64], p: &[f64], e: &EdgePair, alpha: f64, gi: usize) -> f64 {
    let mut d_int_fij = 0.0;
    if gi == 0 {
        let mut r = e.ci - e.cj;
        if r.mag() < SMALL {
            r = e.di * (alpha * e.mag_si);
        }
        let v = e.cosij * r.dot(r).r_ln() * e.mag_si * e.mag_sj;
        // `max(x, 0)` of OpenFOAM: `(x > 0) ? x : 0`
        d_int_fij = if v > 0.0 { v } else { 0.0 };
    } else {
        let pi: Vec<Vector3> = (0..w.len())
            .map(|k| e.ci + e.di * (p[k] * (e.mag_si / 2.0)))
            .collect();
        let pj: Vec<Vector3> = (0..w.len())
            .map(|k| e.cj + e.dj * (p[k] * (e.mag_sj / 2.0)))
            .collect();
        for i in 0..w.len() {
            for j in 0..w.len() {
                let mut r = pi[i] - pj[j];
                if r.mag() < SMALL {
                    r = e.di * (alpha * e.mag_si);
                }
                d_int_fij += e.cosij * w[i] * w[j] * r.dot(r).r_ln();
            }
        }
        d_int_fij *= (e.mag_si / 2.0) * (e.mag_sj / 2.0);
    }
    d_int_fij
}

/// One wall triangle with the index of the (coarse) face it belongs to.
struct WallTriangle {
    a: Vector3,
    b: Vector3,
    c: Vector3,
    face: usize,
}

/// `indexedOctreeBase::perturbTol_ = 10*SMALL`: the tolerance `triSurfaceSearch` hands to
/// `treeDataTriSurface` as `planarTol_` and `triangle::intersection` applies to `u`, `v`, `t`.
const PLANAR_TOL: f64 = 10.0 * SMALL;

/// `triangle::intersection(start, dir, HALF_RAY, tol)` followed by upstream's `distance() <= 1`
/// test (`treeDataPrimitivePatch::findIntersection`): `(t, hit point)` of the segment
/// `start .. start + dir` with the triangle.
fn segment_triangle_hit(
    start: Vector3,
    end: Vector3,
    tri: &WallTriangle,
) -> Option<(f64, Vector3)> {
    let dir = end - start;
    let edge1 = tri.b - tri.a;
    let edge2 = tri.c - tri.a;
    let p_vec = dir.cross(edge2);
    let det = edge1.dot(p_vec);
    if det > -ROOT_VSMALL && det < ROOT_VSMALL {
        return None;
    }
    let inv_det = 1.0 / det;
    let t_vec = start - tri.a;
    let u = t_vec.dot(p_vec) * inv_det;
    if u < -PLANAR_TOL || u > 1.0 + PLANAR_TOL {
        return None;
    }
    let q_vec = t_vec.cross(edge1);
    let v = dir.dot(q_vec) * inv_det;
    if v < -PLANAR_TOL || u + v > 1.0 + PLANAR_TOL {
        return None;
    }
    let t = edge2.dot(q_vec) * inv_det;
    if t < -PLANAR_TOL || t > 1.0 {
        return None;
    }
    Some((t, tri.a + edge1 * u + edge2 * v))
}

/// The wall triangles as the shapes of an `indexedOctree<treeDataTriSurface>`.
struct WallShapes {
    tris: Vec<WallTriangle>,
}

fn arr(p: Vector3) -> V {
    [p.x, p.y, p.z]
}

impl OctreeShapes for WallShapes {
    fn len(&self) -> usize {
        self.tris.len()
    }

    /// `treeDataPrimitivePatch::overlaps(index, searchBox)`: quick bound-box rejection, then the
    /// triangle/box test.
    fn overlaps_box(&self, index: usize, bb: &TreeBox) -> bool {
        let t = &self.tris[index];
        let (a, b, c) = (arr(t.a), arr(t.b), arr(t.c));
        for d in 0..3 {
            let lo = a[d].min(b[d]).min(c[d]);
            let hi = a[d].max(b[d]).max(c[d]);
            if !(bb.min[d] <= hi && lo <= bb.max[d]) {
                return false;
            }
        }
        bb.intersects_triangle(a, b, c)
    }
}

impl LineShapes for WallShapes {
    /// `treeDataPrimitivePatch::findIntersection`.
    fn find_intersection(&self, index: usize, start: V, end: V) -> Option<V> {
        let s = Vector3::new(start[0], start[1], start[2]);
        let e = Vector3::new(end[0], end[1], end[2]);
        segment_triangle_hit(s, e, &self.tris[index]).map(|(_, p)| arr(p))
    }
}

/// The `triSurfaceSearch::tree()` of the wall surface: `maxTreeDepth 10`, leaf size 10, duplicity
/// 3, bounds of the used points inflated by `Random(65431)` (1e-4, ROOTVSMALL).
fn wall_tree(tris: Vec<WallTriangle>) -> Octree<WallShapes> {
    let mut pts: Vec<V> = Vec::with_capacity(3 * tris.len());
    for t in &tris {
        pts.push(arr(t.a));
        pts.push(arr(t.b));
        pts.push(arr(t.c));
    }
    let bb = TreeBox::search_domain(&pts);
    Octree::new(WallShapes { tris }, bb, 10, 10, 3.0)
}

/// `distributedTriSurfaceMesh::findLine` of one ray (serial: `triSurfaceMesh::findLine`, i.e.
/// `indexedOctree::findLine`): the face and point of the first hit.
fn nearest_hit(start: Vector3, end: Vector3, tree: &Octree<WallShapes>) -> Option<(usize, Vector3)> {
    tree.find_line(arr(start), arr(end)).map(|h| {
        let p = h.point;
        (tree.shapes().tris[h.index].face, Vector3::new(p[0], p[1], p[2]))
    })
}

/// `shootRays.H` for one candidate ray: is `target` visible from `source`?
fn ray_is_visible(
    source: usize,
    target: usize,
    start: Vector3,
    end: Vector3,
    tris: &Octree<WallShapes>,
) -> bool {
    let mut cur_start = start;
    let mut cur_end = end;
    // pass 1 handles the first trace; each continuation is one more pass, at most 10
    for pass in 0..10 {
        match nearest_hit(cur_start, cur_end, tris) {
            None => {
                // a continued ray that hits nothing is dropped (upstream: only hits are kept),
                // the very first trace that hits nothing is a visible pair
                return pass == 0;
            }
            Some((face, p)) => {
                if face == source {
                    let d = end - p;
                    cur_start = p + d * 0.01;
                    cur_end = p + d * 1.01;
                } else {
                    return face == target && pass > 0;
                }
            }
        }
    }
    false
}

/// Everything `viewFactorsGen` builds before it shoots rays.
struct Prepared {
    /// Vertex loops of the wall faces, patch by patch.
    face_verts: Vec<Vec<usize>>,
    /// Mesh `Cf` (`primitiveMesh::makeFaceCentresAndAreas`): used by the quadrature.
    cf_mesh: Vec<Vector3>,
    /// Face area vectors `Sf`.
    sf: Vec<Vector3>,
    /// `face::centre`: the coarse-face centre used by the candidate rays.
    cf: Vec<Vector3>,
    /// The octree over the wall triangles.
    tree: Octree<WallShapes>,
}

/// Validate the input, compute the face geometry and build the wall octree.
fn prepare_wall(input: &ViewFactorGenInput) -> Result<Prepared, String> {
    let mut face_verts: Vec<Vec<usize>> = Vec::new();
    for patch in &input.patch_faces {
        for f in patch {
            if f.len() < 3 || f.iter().any(|&v| v >= input.points.len()) {
                return Err(format!("bad face {f:?}"));
            }
            face_verts.push(f.clone());
        }
    }
    let n = face_verts.len();
    // mesh Cf/Sf (primitiveMesh): the fine values used by the quadrature
    let mut cf_mesh = Vec::with_capacity(n);
    let mut sf = Vec::with_capacity(n);
    // face::centre: the coarse-face centre used by the candidate rays
    let mut cf = Vec::with_capacity(n);
    for f in &face_verts {
        let (c, s) = face_centre_and_area(&input.points, f);
        cf_mesh.push(c);
        sf.push(s);
        cf.push(face_centre_of(&input.points, f));
    }
    let mut tri_list = Vec::new();
    for (k, f) in face_verts.iter().enumerate() {
        for t in face_triangles(&input.points, f) {
            tri_list.push(WallTriangle {
                a: input.points[t[0]],
                b: input.points[t[1]],
                c: input.points[t[2]],
                face: k,
            });
        }
    }

    let tree = wall_tree(tri_list);
    Ok(Prepared { face_verts, cf_mesh, sf, cf, tree })
}

/// One candidate ray of `shootRays.H` with the result of its first `findLine`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayFirstHit {
    /// Source face (local index).
    pub source: usize,
    /// Target face.
    pub target: usize,
    /// Ray start `fc + 0.001 d`.
    pub start: Vector3,
    /// Ray end `fc + 0.999 d`.
    pub end: Vector3,
    /// First hit of the segment with the wall: `(face, point)`, `None` for a miss.
    pub hit: Option<(usize, Vector3)>,
}

/// Every candidate ray (`(d & fA) < 0 && (d & remA) > 0`), in upstream's order (source ascending,
/// target ascending), with its first `findLine` result. Diagnostic for comparing with
/// upstream's `findLine` ray by ray (gh:#1564).
///
/// # Errors
/// As [`generate_view_factors`].
pub fn trace_candidate_rays(input: &ViewFactorGenInput) -> Result<Vec<RayFirstHit>, String> {
    let w = prepare_wall(input)?;
    let n = w.face_verts.len();
    let mut out = Vec::new();
    for i in 0..n {
        let fc = w.cf[i];
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = w.cf[j] - fc;
            if d.dot(w.sf[i]) < 0.0 && d.dot(w.sf[j]) > 0.0 {
                let start = fc + d * 0.001;
                let end = fc + d * 0.999;
                let hit = nearest_hit(start, end, &w.tree);
                out.push(RayFirstHit { source: i, target: j, start, end, hit });
            }
        }
    }
    Ok(out)
}

/// Generate `F` and `globalFaceFaces` for un-agglomerated wall patches (3-D).
///
/// `viewFactorsGen` with no `finalAgglom` in `constant/`, serial, `-DNO_CGAL`.
///
/// # Errors
/// Returns a message when a face has fewer than 3 vertices or a vertex index is out of range.
pub fn generate_view_factors(
    input: &ViewFactorGenInput,
    settings: ViewFactorGenSettings,
) -> Result<GeneratedViewFactors, String> {
    let Prepared { face_verts, cf_mesh, sf, cf, tree: tris } = prepare_wall(input)?;
    let n = face_verts.len();
    // candidate rays and visibility, row by row, ascending target
    let mut visible: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        let fc = cf[i];
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = cf[j] - fc;
            if d.dot(sf[i]) < 0.0 && d.dot(sf[j]) > 0.0 {
                let start = fc + d * 0.001;
                let end = fc + d * 0.999;
                if ray_is_visible(i, j, start, end, &tris) {
                    visible[i].push(j);
                }
            }
        }
    }

    let (gp, gw) = gauss_tables();
    let mut f_out: Vec<Vec<f64>> = vec![Vec::new(); n];
    for i in 0..n {
        let a_i = sf[i];
        let mag_ai = a_i.mag();
        let l_points: Vec<Vector3> = face_verts[i].iter().map(|&v| input.points[v]).collect();
        for &j in &visible[i] {
            let mut far = false;
            let d_ai = (sf[i].mag() / PI).sqrt();
            let d_aj = (sf[j].mag() / PI).sqrt();
            let dist = (cf_mesh[i] - cf_mesh[j]).mag() / ((d_ai + d_aj) / 2.0);
            if dist > settings.dist_tol {
                far = true;
            }
            let value = if far {
                let f2ai = view_factor_2ai(cf_mesh[i], cf_mesh[j], sf[i], sf[j]);
                f2ai / mag_ai
            } else {
                let r_points: Vec<Vector3> =
                    face_verts[j].iter().map(|&v| input.points[v]).collect();
                two_li(&l_points, &r_points, mag_ai, settings, &gp, &gw)
            };
            f_out[i].push(value);
        }
    }
    Ok(GeneratedViewFactors {
        f: f_out,
        global_face_faces: visible,
    })
}

/// The 2LI branch of `viewFactorsGen.C` for one face pair.
fn two_li(
    l_points: &[Vector3],
    r_points: &[Vector3],
    mag_ai: f64,
    settings: ViewFactorGenSettings,
    gp: &[Vec<f64>; 5],
    gw: &[Vec<f64>; 5],
) -> f64 {
    let n_local = l_points.len();
    let n_remote = r_points.len();
    let mut old = 0.0;
    let mut result = 0.0;
    for gi in 0..5 {
        let mut f2li = 0.0;
        for i in 0..n_local {
            let (si, ci) = if i == 0 {
                (
                    l_points[i] - l_points[n_local - 1],
                    (l_points[i] + l_points[n_local - 1]) / 2.0,
                )
            } else {
                (
                    l_points[i] - l_points[i - 1],
                    (l_points[i] + l_points[i - 1]) / 2.0,
                )
            };
            for j in 0..n_remote {
                let (sj, cj) = if j == 0 {
                    (
                        r_points[j] - r_points[n_remote - 1],
                        (r_points[j] + r_points[n_remote - 1]) / 2.0,
                    )
                } else {
                    (
                        r_points[j] - r_points[j - 1],
                        (r_points[j] + r_points[j - 1]) / 2.0,
                    )
                };
                let mag_si = si.mag();
                let mag_sj = sj.mag();
                let cosij = si.dot(sj) / (mag_si * mag_sj);
                let di = si / mag_si;
                let dj = sj / mag_sj;
                let mut quad_order = gi;
                let r = ci - cj;
                if r.mag() < SMALL {
                    quad_order = 0;
                }
                let e = EdgePair {
                    mag_si,
                    mag_sj,
                    di,
                    dj,
                    ci,
                    cj,
                    cosij,
                };
                // upstream passes GaussWeights[gi]/GaussPoints[gi] with the (possibly reset) order
                f2li += gauss_quad(&gw[gi], &gp[gi], &e, settings.alpha, quad_order);
            }
        }
        let err = if f2li.abs() > ROOT_VSMALL {
            (f2li - old) / f2li
        } else {
            0.0
        };
        if (err.abs() < settings.gauss_quad_tol && gi > 0) || gi == 4 {
            result = f2li / mag_ai / 4.0 / PI;
            break;
        }
        old = f2li;
    }
    result
}

/// `%.17g` as printed by an `ostream` with `precision(17)` and default floatfield.
#[must_use]
pub fn format_g17(x: f64) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let sci = format!("{:.16e}", x); // d.dddddddddddddddde[-]X
    let (mant, exp) = sci.split_once('e').expect("exp form");
    let exp: i32 = exp.parse().expect("exp");
    if !(-4..17).contains(&exp) {
        let m = trim_zeros(mant);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{m}e{sign}{:02}", exp.abs());
    }
    let digits = (16 - exp) as usize;
    trim_zeros(&format!("{:.*}", digits, x))
}

fn trim_zeros(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

const BANNER: &str =
    "/*--------------------------------*- C++ -*----------------------------------*\\\n\
| =========                 |                                                 |\n\
| \\\\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox           |\n\
|  \\\\    /   O peration     | Version:  2506                                  |\n\
|   \\\\  /    A nd           | Website:  www.openfoam.com                      |\n\
|    \\\\/     M anipulation  |                                                 |\n\
\\*---------------------------------------------------------------------------*/\n";

const FOOTER: &str =
    "\n\n// ************************************************************************* //\n";

fn header(class: &str, object: &str) -> String {
    format!(
        "{BANNER}FoamFile\n{{\n    version     2.0;\n    format      ascii;\n    arch        \"LSB;label=32;scalar=64\";\n    class       {class};\n    location    \"constant\";\n    object      {object};\n}}\n// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //\n"
    )
}

/// The `F` file (`scalarListIOList`, `writePrecision 17`) as upstream writes it.
#[must_use]
pub fn write_scalar_list_list_file(rows: &[Vec<f64>]) -> String {
    let mut s = header("scalarListList", "F");
    s.push_str(&format!("\n\n{}\n(\n", rows.len()));
    for r in rows {
        s.push_str(&format!("\n{}\n(\n", r.len()));
        for v in r {
            s.push_str(&format_g17(*v));
            s.push('\n');
        }
        s.push_str(")\n\n");
    }
    s.push_str(")\n");
    s.push_str(FOOTER);
    s
}

/// The `globalFaceFaces` file (`labelListIOList`) as upstream writes it.
#[must_use]
pub fn write_label_list_list_file(rows: &[Vec<usize>]) -> String {
    let mut s = header("labelListList", "globalFaceFaces");
    s.push_str(&format!("\n\n{}\n(\n", rows.len()));
    for r in rows {
        s.push_str(&format!("\n{}\n(\n", r.len()));
        for v in r {
            s.push_str(&format!("{v}\n"));
        }
        s.push_str(")\n\n");
    }
    s.push_str(")\n");
    s.push_str(FOOTER);
    s
}

/// The `mapDist` file (`IOmapDistribute`) of a serial run: identity maps of size `n`.
#[must_use]
pub fn write_map_dist_file(n: usize) -> String {
    let ids: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    let list = format!("{n}({})", ids.join(" "));
    let mut s = header("mapDistribute", "mapDist");
    s.push_str(&format!(
        "\n{n}\n\n1\n(\n{list}\n)\n\n1\n(\n{list}\n)\n0 0 0\n\n0()\n0()\n"
    ));
    s.push_str(
        "\n// ************************************************************************* //\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box(lx: f64, ly: f64, lz: f64) -> ViewFactorGenInput {
        let p = |x: f64, y: f64, z: f64| Vector3::new(x * lx, y * ly, z * lz);
        let points = vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(1., 1., 0.),
            p(0., 1., 0.),
            p(0., 0., 1.),
            p(1., 0., 1.),
            p(1., 1., 1.),
            p(0., 1., 1.),
        ];
        // blockMesh outward-facing quads
        let faces = [
            [0, 4, 7, 3],
            [1, 2, 6, 5],
            [0, 1, 5, 4],
            [3, 7, 6, 2],
            [0, 3, 2, 1],
            [4, 5, 6, 7],
        ];
        ViewFactorGenInput {
            points,
            patch_faces: faces.iter().map(|f| vec![f.to_vec()]).collect(),
        }
    }

    #[test]
    fn cube_six_faces_unity_row_sums_and_symmetry() {
        let g = generate_view_factors(&unit_box(1.0, 1.0, 1.0), ViewFactorGenSettings::default())
            .unwrap();
        for i in 0..6 {
            let s: f64 = g.f[i].iter().sum();
            assert!((s - 1.0).abs() < 0.05, "row {i} sum {s}");
            assert_eq!(g.global_face_faces[i].len(), 5);
        }
        // opposite faces of a unit cube: 0.19982 (Howell C-11)
        let f_opp = g.f[0][g.global_face_faces[0].iter().position(|&j| j == 1).unwrap()];
        assert!((f_opp - 0.19982).abs() < 0.02, "{f_opp}");
    }

    #[test]
    fn baffle_blocks_the_ray_but_not_other_pairs() {
        // two big parallel squares with a small square wall between them
        let mut inp = unit_box(1.0, 1.0, 1.0);
        let base = inp.points.len();
        let (z, h) = (0.5, 0.2);
        inp.points.push(Vector3::new(0.5 - h, 0.5 - h, z));
        inp.points.push(Vector3::new(0.5 + h, 0.5 - h, z));
        inp.points.push(Vector3::new(0.5 + h, 0.5 + h, z));
        inp.points.push(Vector3::new(0.5 - h, 0.5 + h, z));
        // a baffle facing +z; the target of the candidate test sees only its facing side
        inp.patch_faces
            .push(vec![vec![base, base + 1, base + 2, base + 3]]);
        let g = generate_view_factors(&inp, ViewFactorGenSettings::default()).unwrap();
        // zmin (patch 4) face index 4 sees zmax (index 5) in the open box; the baffle blocks the
        // centre-to-centre ray, so 5 is not in its list
        assert!(!g.global_face_faces[4].contains(&5));
        assert!(g.global_face_faces[0].contains(&1));
    }

    #[test]
    fn g17_formatting_matches_printf() {
        assert_eq!(format_g17(0.1), "0.10000000000000001");
        assert_eq!(format_g17(0.019106960645959528), "0.019106960645959528");
        assert_eq!(format_g17(1.0), "1");
        assert_eq!(format_g17(1e-5), "1.0000000000000001e-05");
        assert_eq!(format_g17(0.0), "0");
        assert_eq!(format_g17(2.5), "2.5");
    }

    #[test]
    fn map_dist_and_lists_round_trip_structure() {
        let t = write_map_dist_file(3);
        assert!(t.contains("3(0 1 2)"));
        let l = write_label_list_list_file(&[vec![1, 2], vec![]]);
        assert!(l.contains("\n2\n(\n1\n2\n)\n"));
        let f = write_scalar_list_list_file(&[vec![0.5]]);
        assert!(f.contains("0.5\n)"));
    }
}
