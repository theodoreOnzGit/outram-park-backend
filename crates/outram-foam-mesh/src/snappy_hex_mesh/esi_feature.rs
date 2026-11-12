// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), Copyright (C) 2011-2016 OpenFOAM
// Foundation, 2015-2026 OpenCFD Ltd (ESI), GPL-3.0-or-later.

//! Explicit feature snapping of ESI v2506 `snappyHexMesh`, translated (gh:#1467, epic
//! #1459 / #1336, round 6).
//!
//! **Upstream (OpenFOAM.com ESI v2506):** `snappyHexMeshDriver/snappySnapDriverFeature.C`
//! (`calcNearestFace` 223, `calcNearestFacePointProperties` 476, `findNormal` 819,
//! `findMultiPatchPoint` 852, `writeStats` 932, per-point `featureAttractionUsingReconstruction`
//! 980, `stringFeatureEdges` 1284, `findNearFeatureEdge` 2251, `findNearFeaturePoint` 2316,
//! `determineFeatures` 2424, `determineBaffleFeatures` 2990, `reverseAttractMeshPoints` 3330,
//! `featureAttractionUsingFeatureEdges` 3608, `preventFaceSqueeze` 3915,
//! `calcNearestSurfaceFeature` 4049, `smoothAndConstrain` 132);
//! `snappySnapDriver.C::doSnap` (2574, the morph loop 3305-3470);
//! `refinementFeatures.C` (`findNearestEdge` 632, `findNearestRegionEdge` 688,
//! `findNearestPoint` 796); `OpenFOAM/meshes/primitiveShapes/plane/plane.C`
//! (`planeIntersect`, `planePlaneIntersect`, `planeCoeffs`), `TensorI.H` (`adjunct`, `det`,
//! `inv`), `pointConstraintI.H` (`applyConstraint`), `line/lineI.H` (`nearestDist`,
//! `unitVec`), `triangle/triangleI.H` (`nearestPointClassify`, `unitNormal`),
//! `PrimitivePatchAddressing.C` (`calcAddressing`: patch edge order),
//! `PatchToolsNormals.C` (`pointNormals`), `treeDataEdge.C`, `treeDataPoint.C`,
//! `treeDataPrimitivePatch.C` (`findNearest`: `sqr(nearHit.distance())`).
//!
//! **Scope.** Explicit feature snapping with an `extendedFeatureEdgeMesh`, the
//! `multiRegionFeatureSnap` branches, `stringFeatures`, `detectBaffles` (baffle edges attract
//! their end points to the nearest feature edge, `baffleFeaturePoints false`; round 7), no face
//! splitting (`nFaceSplitInterval` unset), `releasePoints false`, `avoidDiagonal false`,
//! `strictRegionSnap false`, serial, one unzoned surface. NOT ported: `implicitFeatureSnap`
//! driver, `releasePointsNextToMultiPatch`, `avoidDiagonalAttraction`, `splitDiagonals`,
//! `baffleFeaturePoints true`, and `detectNearSurfaces` (ESI overrides the nearest displacement
//! of 2 points in iteration 9 of the flange; both points are feature-constrained at blend 1,
//! so the result is bit-identical without it, see the V&V record, P47).
//!
//! **Nearest-shape searches** walk upstream's `indexedOctree` ([`crate::snappy_hex_mesh::esi_octree`])
//! so equidistant shapes resolve as in ESI (round 7). ~~Round 6 searched by brute force, first
//! minimum in index order~~ **CORRECTED 2026-11-12**: that choice picked a different feature
//! edge than ESI at four equidistant-edge points and was the first divergence of the smoothed
//! displacement. The brute-force searches remain as `*_brute` twins for comparison tests.
//!
//! Numerics: plain IEEE `+ - * /` and `sqrt` in upstream's operand grouping, no `mul_add`.

use crate::check_mesh::esi_face_area_normal;
use crate::snappy_hex_mesh::esi_snap::{
    add, cross, displacement_field, div, dot, face_centre, mag, mag_sqr, scale, scale_mesh_loop, smooth_displacement, sub, MeshEdges,
    MotionSmoother, PatchAddr, ScaleControls, ScaleIteration, ScaleTopology, SnapMesh, V,
};
use crate::snappy_hex_mesh::stl::{StlZoned, Triangle};
use crate::snappy_hex_mesh::esi_octree::{edge_tree, surface_tree, EdgeShapes, Octree, OctreeShapes, TriShapes};
use outram_foam_basic_lib::primitives::Vector3;


const VSMALL: f64 = 1e-300;
const ROOTVSMALL: f64 = 1e-150;
const GREAT: f64 = 1e15;

/// Errors of the feature-snap stages.
#[derive(Debug, Clone, PartialEq)]
pub enum FeatureError {
    /// The `extendedFeatureEdgeMesh` text could not be read.
    BadEdgeMesh(String),
    /// A situation upstream handles that this port does not (`baffleFeaturePoints`, open patch
    /// edges, ...). Nothing is approximated.
    NotPorted(String),
}

// ---------------------------------------------------------------------------------------
// extendedFeatureEdgeMesh
// ---------------------------------------------------------------------------------------

/// The parts of an `extendedFeatureEdgeMesh` the snapping reads: points, edges, the
/// feature-point partition and the region edges.
#[derive(Debug, Clone)]
pub struct ExtendedEdgeMesh {
    /// Points (feature points first: convex, concave, mixed; then non-feature points).
    pub points: Vec<V>,
    /// Edges as point pairs.
    pub edges: Vec<[usize; 2]>,
    /// `concaveStart`.
    pub concave_start: usize,
    /// `mixedStart`.
    pub mixed_start: usize,
    /// `nonFeatureStart`: points `0..non_feature_start` are the feature points.
    pub non_feature_start: usize,
    /// `regionEdges`: edges between two STL regions.
    pub region_edges: Vec<usize>,
}

fn section<'a>(text: &'a str, header: &str) -> Result<&'a str, FeatureError> {
    let key = format!("\n// {header}\n");
    let at = text.find(&key).ok_or_else(|| FeatureError::BadEdgeMesh(format!("no section {header}")))?;
    Ok(&text[at + key.len()..])
}

fn list_body(s: &str) -> Result<(usize, &str), FeatureError> {
    let s = s.trim_start();
    let nl = s.find('\n').ok_or_else(|| FeatureError::BadEdgeMesh("truncated".into()))?;
    let n: usize = s[..nl].trim().parse().map_err(|_| FeatureError::BadEdgeMesh("list size".into()))?;
    let rest = &s[nl + 1..];
    let open = rest.find('(').ok_or_else(|| FeatureError::BadEdgeMesh("no (".into()))?;
    let body = &rest[open + 1..];
    Ok((n, body))
}

/// Read an ASCII `extendedFeatureEdgeMesh` file (`extendedEdgeMesh::read`).
///
/// # Errors
/// [`FeatureError::BadEdgeMesh`] when a section is missing or malformed.
pub fn read_extended_edge_mesh(text: &str) -> Result<ExtendedEdgeMesh, FeatureError> {
    let bad = |m: &str| FeatureError::BadEdgeMesh(m.to_string());
    let (np, body) = list_body(section(text, "points")?)?;
    let mut points = Vec::with_capacity(np);
    for line in body.lines().filter(|l| l.starts_with('(')).take(np) {
        let v: Vec<f64> = line
            .trim_matches(|c| c == '(' || c == ')')
            .split_whitespace()
            .map(|x| x.parse::<f64>().map_err(|_| bad("point")))
            .collect::<Result<_, _>>()?;
        if v.len() != 3 {
            return Err(bad("point arity"));
        }
        points.push([v[0], v[1], v[2]]);
    }
    if points.len() != np {
        return Err(bad("point count"));
    }
    let (ne, body) = list_body(section(text, "edges")?)?;
    let mut edges = Vec::with_capacity(ne);
    for line in body.lines().filter(|l| l.starts_with('(')).take(ne) {
        let v: Vec<usize> = line
            .trim_matches(|c| c == '(' || c == ')')
            .split_whitespace()
            .map(|x| x.parse::<usize>().map_err(|_| bad("edge")))
            .collect::<Result<_, _>>()?;
        if v.len() != 2 {
            return Err(bad("edge arity"));
        }
        edges.push([v[0], v[1]]);
    }
    if edges.len() != ne {
        return Err(bad("edge count"));
    }
    let starts = section(text, "concaveStart mixedStart nonFeatureStart")?;
    let s: Vec<usize> = starts.lines().next().ok_or_else(|| bad("starts"))?.split_whitespace().map(|x| x.parse().unwrap_or(usize::MAX)).collect();
    if s.len() != 3 {
        return Err(bad("starts arity"));
    }
    let (nr, body) = list_body(section(text, "regionEdges")?)?;
    let region_edges: Vec<usize> = body
        .lines()
        .take_while(|l| !l.starts_with(')'))
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse::<usize>().map_err(|_| bad("region edge")))
        .collect::<Result<_, _>>()?;
    if region_edges.len() != nr {
        return Err(bad("region edge count"));
    }
    Ok(ExtendedEdgeMesh { points, edges, concave_start: s[0], mixed_start: s[1], non_feature_start: s[2], region_edges })
}

// ---------------------------------------------------------------------------------------
// plane, tensor inverse, pointConstraint
// ---------------------------------------------------------------------------------------

/// `Foam::plane` (point and unit normal).
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    normal: V,
    origin: V,
}

impl Plane {
    /// `plane(origin, normal)`: the normal is divided by its magnitude (`makeUnitNormal`).
    pub fn new(origin: V, normal: V) -> Self {
        let m = mag(normal);
        Self { normal: div(normal, m), origin }
    }

    /// `planeCoeffs()`.
    pub fn coeffs(&self) -> [f64; 4] {
        let n = self.normal;
        let (mx, my, mz) = (n[0].abs(), n[1].abs(), n[2].abs());
        let mut c = [0.0; 4];
        if mx > my {
            if mx > mz {
                c[0] = 1.0;
                c[1] = n[1] / n[0];
                c[2] = n[2] / n[0];
            } else {
                c[0] = n[0] / n[2];
                c[1] = n[1] / n[2];
                c[2] = 1.0;
            }
        } else if my > mz {
            c[0] = n[0] / n[1];
            c[1] = 1.0;
            c[2] = n[2] / n[1];
        } else {
            c[0] = n[0] / n[2];
            c[1] = n[1] / n[2];
            c[2] = 1.0;
        }
        c[3] = -c[0] * self.origin[0] - c[1] * self.origin[1] - c[2] * self.origin[2];
        c
    }

    /// `planeIntersect(plane2)`: a point on the line of intersection and its direction
    /// (not normalised).
    pub fn plane_intersect(&self, p2: &Plane) -> (V, V) {
        let (n1, n2) = (self.normal, p2.normal);
        let (q1, q2) = (self.origin, p2.origin);
        let n1p1 = dot(n1, q1);
        let n2p2 = dot(n2, q2);
        let dir = cross(n1, n2);
        let (mx, my, mz) = (dir[0].abs(), dir[1].abs(), dir[2].abs());
        let (i_zero, i1, i2) = if mx > my {
            if mx > mz {
                (0, 1, 2)
            } else {
                (2, 0, 1)
            }
        } else if my > mz {
            (1, 2, 0)
        } else {
            (2, 0, 1)
        };
        let mut pt = [0.0; 3];
        pt[i_zero] = 0.0;
        pt[i1] = (n2[i2] * n1p1 - n1[i2] * n2p2) / (n1[i1] * n2[i2] - n2[i1] * n1[i2]);
        pt[i2] = (n2[i1] * n1p1 - n1[i1] * n2p2) / (n1[i2] * n2[i1] - n1[i1] * n2[i2]);
        (pt, dir)
    }

    /// `planePlaneIntersect(plane2, plane3)`: `inv(A) & (-b)` with `inv = adjunct()/det()`.
    pub fn plane_plane_intersect(&self, p2: &Plane, p3: &Plane) -> V {
        let (c1, c2, c3) = (self.coeffs(), p2.coeffs(), p3.coeffs());
        let (xx, xy, xz) = (c1[0], c1[1], c1[2]);
        let (yx, yy, yz) = (c2[0], c2[1], c2[2]);
        let (zx, zy, zz) = (c3[0], c3[1], c3[2]);
        let det = xx * yy * zz + xy * yz * zx + xz * yx * zy - xx * yz * zy - xy * yx * zz - xz * yy * zx;
        let adj = [
            [yy * zz - zy * yz, xz * zy - xy * zz, xy * yz - xz * yy],
            [zx * yz - yx * zz, xx * zz - xz * zx, yx * xz - xx * yz],
            [yx * zy - yy * zx, xy * zx - xx * zy, xx * yy - yx * xy],
        ];
        let inv = [
            [adj[0][0] / det, adj[0][1] / det, adj[0][2] / det],
            [adj[1][0] / det, adj[1][1] / det, adj[1][2] / det],
            [adj[2][0] / det, adj[2][1] / det, adj[2][2] / det],
        ];
        let v = [-c1[3], -c2[3], -c3[3]];
        [
            inv[0][0] * v[0] + inv[0][1] * v[1] + inv[0][2] * v[2],
            inv[1][0] * v[0] + inv[1][1] * v[1] + inv[1][2] * v[2],
            inv[2][0] * v[0] + inv[2][1] * v[1] + inv[2][2] * v[2],
        ]
    }
}

/// `pointConstraint`: number of constrained directions (0 free, 1 plane, 2 line, 3 point)
/// and the plane normal or line direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointConstraint {
    /// `first()`.
    pub n: i32,
    /// `second()`.
    pub dir: V,
}

impl PointConstraint {
    /// The free point.
    pub fn none() -> Self {
        Self { n: 0, dir: [0.0; 3] }
    }

    /// `applyConstraint(cd)`.
    pub fn apply_constraint(&mut self, cd: V) {
        if self.n == 0 {
            self.n = 1;
            self.dir = cd;
        } else if self.n == 1 {
            let plane_normal = cross(cd, self.dir);
            let m = mag(plane_normal);
            if m > 1e-3 {
                self.n = 2;
                self.dir = div(plane_normal, m);
            }
        } else if self.n == 2 && mag(vdot(cd, self.dir)) > 1e-3 {
            self.n = 3;
            self.dir = [0.0; 3];
        }
    }
}

fn vdot(a: V, b: V) -> V {
    [dot(a, b), 0.0, 0.0]
}

// ---------------------------------------------------------------------------------------
// patch topology (PrimitivePatch::calcAddressing order)
// ---------------------------------------------------------------------------------------

/// Patch edges in `PrimitivePatch::calcAddressing` order, their faces, and `pointEdges`.
#[derive(Debug, Clone)]
pub struct PatchTopo {
    /// Edges (local point pairs); internal edges (two or more faces) first.
    pub edges: Vec<[usize; 2]>,
    /// Faces using each edge.
    pub edge_faces: Vec<Vec<usize>>,
    /// Edges using each local point, ascending.
    pub point_edges: Vec<Vec<usize>>,
    /// Number of internal edges.
    pub n_internal_edges: usize,
}

fn same_edge(a: [usize; 2], b: [usize; 2]) -> bool {
    (a[0] == b[0] && a[1] == b[1]) || (a[0] == b[1] && a[1] == b[0])
}

/// `PrimitivePatch::calcAddressing` (edges, edgeFaces) and `pointEdges`.
pub fn patch_topology(pa: &PatchAddr) -> PatchTopo {
    let lf = &pa.local_faces;
    let face_into_edges: Vec<Vec<[usize; 2]>> =
        lf.iter().map(|f| (0..f.len()).map(|i| [f[i], f[(i + 1) % f.len()]]).collect()).collect();
    let mut face_edges: Vec<Vec<i64>> = lf.iter().map(|f| vec![-1; f.len()]).collect();
    let mut edges: Vec<[usize; 2]> = Vec::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();
    for facei in 0..lf.len() {
        let cur_edges = &face_into_edges[facei];
        let mut nei_faces: Vec<Vec<usize>> = vec![Vec::new(); lf[facei].len()];
        let mut edge_of_nei_face: Vec<Vec<usize>> = vec![Vec::new(); lf[facei].len()];
        let mut n_neighbours = 0;
        for edge_i in 0..cur_edges.len() {
            if face_edges[facei][edge_i] >= 0 {
                continue;
            }
            let e = cur_edges[edge_i];
            let mut found = false;
            for &cur_nei in &pa.point_faces[e[0]] {
                if cur_nei > facei {
                    for (nei_edge_i, se) in face_into_edges[cur_nei].iter().enumerate() {
                        if same_edge(*se, e) {
                            found = true;
                            nei_faces[edge_i].push(cur_nei);
                            edge_of_nei_face[edge_i].push(nei_edge_i);
                        }
                    }
                }
            }
            if found {
                n_neighbours += 1;
            }
        }
        for _ in 0..n_neighbours {
            let mut next_nei: Option<usize> = None;
            let mut min_nei = lf.len();
            for (nf_i, nf) in nei_faces.iter().enumerate() {
                if let Some(&first) = nf.first() {
                    if first < min_nei {
                        next_nei = Some(nf_i);
                        min_nei = first;
                    }
                }
            }
            let nn = next_nei.expect("internal edge insertion");
            let n_edges = edges.len() as i64;
            edges.push(cur_edges[nn]);
            face_edges[facei][nn] = n_edges;
            let mut cur_ef = vec![facei];
            for (k, &nf) in nei_faces[nn].iter().enumerate() {
                face_edges[nf][edge_of_nei_face[nn][k]] = n_edges;
                cur_ef.push(nf);
            }
            edge_faces.push(cur_ef);
            nei_faces[nn].clear();
            edge_of_nei_face[nn].clear();
        }
    }
    let n_internal_edges = edges.len();
    for facei in 0..lf.len() {
        for edge_i in 0..face_edges[facei].len() {
            if face_edges[facei][edge_i] < 0 {
                face_edges[facei][edge_i] = edges.len() as i64;
                edges.push(face_into_edges[facei][edge_i]);
                edge_faces.push(vec![facei]);
            }
        }
    }
    let mut point_edges: Vec<Vec<usize>> = vec![Vec::new(); pa.mesh_points.len()];
    for (ei, e) in edges.iter().enumerate() {
        point_edges[e[0]].push(ei);
        point_edges[e[1]].push(ei);
    }
    PatchTopo { edges, edge_faces, point_edges, n_internal_edges }
}

// ---------------------------------------------------------------------------------------
// surface and feature queries
// ---------------------------------------------------------------------------------------

/// The unzoned snapping surface: triangles with the STL region of each (`STLpoint` is
/// single precision: build it from [`crate::snappy_hex_mesh::stl::read_stl_zoned`]).
#[derive(Debug, Clone)]
pub struct FeatureSurface {
    /// Triangles.
    pub tris: Vec<Triangle>,
    /// STL region (zone) of each triangle.
    pub region: Vec<usize>,
    /// `triSurfaceSearch::tree()`.
    pub tree: Octree<TriShapes>,
}

impl FeatureSurface {
    /// From a zoned STL (all solids, single-precision corners).
    pub fn from_zoned(z: &StlZoned) -> Self {
        let tris: Vec<Triangle> = z.triangles.iter().map(|t| Triangle::new(z.points[t[0]], z.points[t[1]], z.points[t[2]])).collect();
        let tree = surface_tree(&tris);
        Self { tris, region: z.zones.clone(), tree }
    }
}

fn v3(p: Vector3) -> V {
    [p.x, p.y, p.z]
}

/// `triangle::unitNormal(a, b, c)`: `0.5*((b-a)^(c-a))` normalised with `ROOTVSMALL`.
fn tri_unit_normal(t: &Triangle) -> V {
    let (a, b, c) = (v3(t.a), v3(t.b), v3(t.c));
    let n = scale(0.5, cross(sub(b, a), sub(c, a)));
    let s = mag(n);
    if s < ROOTVSMALL {
        [0.0; 3]
    } else {
        div(n, s)
    }
}

/// Counts of exact-equal-distance ties met by the brute-force nearest searches (where an
/// octree traversal could pick a different shape); see the module note.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TieCounts {
    /// Triangle searches (face centres).
    pub triangle: usize,
    /// Feature-edge searches.
    pub edge: usize,
    /// Feature-point searches.
    pub feature_point: usize,
    /// Patch-point searches of the reverse attraction.
    pub patch_point: usize,
}

impl FeatureSurface {
    /// Nearest triangle to `p` closer than `dist_sqr` (strict, as `findNearestOp`): the
    /// triangle, the nearest point and the unit normal of the triangle.
    pub fn nearest(&self, p: V, dist_sqr: f64, _ties: &mut TieCounts) -> Option<(usize, V, V)> {
        self.tree.find_nearest(p, dist_sqr).map(|(i, q)| (i, q, tri_unit_normal(&self.tris[i])))
    }

    /// Brute-force nearest triangle (first minimum in index order), the round-6 search.
    pub fn nearest_brute(&self, p: V, dist_sqr: f64, ties: &mut TieCounts) -> Option<(usize, V, V)> {
        let sample = Vector3::new(p[0], p[1], p[2]);
        let mut best = dist_sqr;
        let mut hit: Option<(usize, V)> = None;
        let mut tied = false;
        for (i, t) in self.tris.iter().enumerate() {
            let q = v3(t.closest_point(sample));
            let d = mag(sub(q, p));
            let d2 = d * d;
            if d2 < best {
                best = d2;
                hit = Some((i, q));
                tied = false;
            } else if hit.is_some() && d2 == best {
                tied = true;
            }
        }
        if tied {
            ties.triangle += 1;
        }
        hit.map(|(i, q)| (i, q, tri_unit_normal(&self.tris[i])))
    }
}

/// `calcNearestSurface` with upstream's `sqr(nearHit.distance())` comparison (`treeDataPrimitivePatch::findNearest`):
/// the displacement to the nearest surface point within `snap_dist` (zero where none is)
/// and the number of points with none. Also returns, per point, the nearest surface point
/// and normal (used by `detectNearSurfaces`, not ported).
pub fn calc_nearest_surface_sqr_mag(
    local_points: &[V],
    snap_dist: &[f64],
    surface: &FeatureSurface,
    ties: &mut TieCounts,
) -> (Vec<V>, usize) {
    let mut disp = vec![[0.0; 3]; local_points.len()];
    let mut missing = 0;
    for (i, p) in local_points.iter().enumerate() {
        match surface.nearest(*p, snap_dist[i] * snap_dist[i], ties) {
            Some((_, q, _)) => disp[i] = sub(q, *p),
            None => missing += 1,
        }
    }
    for (i, d) in disp.iter_mut().enumerate() {
        let m = mag(*d);
        if m > snap_dist[i] {
            *d = scale(snap_dist[i] / m, *d);
        }
    }
    (disp, missing)
}

/// One `extendedFeatureEdgeMesh` with the searches `refinementFeatures` offers.
#[derive(Debug, Clone)]
pub struct FeatureSet {
    /// The feature meshes.
    pub features: Vec<ExtendedEdgeMesh>,
    /// `refinementFeatures::edgeTrees_`: per feature, the octree over all edges.
    pub edge_trees: Vec<Octree<EdgeShapes>>,
    /// `refinementFeatures::regionEdgeTrees()`: per feature, the octree over the region edges.
    pub region_edge_trees: Vec<Octree<EdgeShapes>>,
}

impl FeatureSet {
    /// Builds the upstream octrees (`buildTrees`, `regionEdgeTrees`) of every feature.
    pub fn new(features: Vec<ExtendedEdgeMesh>) -> Self {
        let edge_trees = features.iter().map(|em| edge_tree(&em.edges, &em.points, (0..em.edges.len()).collect())).collect();
        let region_edge_trees = features.iter().map(|em| edge_tree(&em.edges, &em.points, em.region_edges.clone())).collect();
        Self { features, edge_trees, region_edge_trees }
    }
}

fn line_nearest(a: V, b: V, p: V) -> (V, f64) {
    let v = sub(b, a);
    let w = sub(p, a);
    let c1 = dot(v, w);
    if c1 <= 0.0 {
        return (a, mag(sub(p, a)));
    }
    let c2 = dot(v, v);
    if c2 <= c1 {
        return (b, mag(sub(p, b)));
    }
    let bb = c1 / c2;
    let pb = add(a, scale(bb, v));
    (pb, mag(sub(p, pb)))
}

fn unit_vec(a: V, b: V) -> V {
    let v = sub(b, a);
    let s = mag(v);
    if s < ROOTVSMALL {
        [0.0; 3]
    } else {
        div(v, s)
    }
}

/// Result of a nearest-edge query: feature index, edge index, point, unit edge direction.
type EdgeHit = Option<(usize, usize, V, V)>;

impl FeatureSet {
    fn nearest_edge_brute(&self, region_only: bool, sample: V, dist_sqr: f64, ties: &mut TieCounts) -> EdgeHit {
        let mut result: EdgeHit = None;
        for (feati, em) in self.features.iter().enumerate() {
            let all: Vec<usize>;
            let list: &[usize] = if region_only {
                &em.region_edges
            } else {
                all = (0..em.edges.len()).collect();
                &all
            };
            if list.is_empty() {
                continue;
            }
            let start = match &result {
                Some((_, _, pt, _)) => mag_sqr(sub(*pt, sample)),
                None => dist_sqr,
            };
            let mut best = start;
            let mut hit: Option<(usize, V)> = None;
            let mut tied = false;
            for &ei in list {
                let e = em.edges[ei];
                let (pt, d) = line_nearest(em.points[e[0]], em.points[e[1]], sample);
                let d2 = d * d;
                if d2 < best {
                    best = d2;
                    hit = Some((ei, pt));
                    tied = false;
                } else if hit.is_some() && d2 == best {
                    tied = true;
                }
            }
            if tied {
                ties.edge += 1;
            }
            if let Some((ei, pt)) = hit {
                let e = em.edges[ei];
                result = Some((feati, ei, pt, unit_vec(em.points[e[0]], em.points[e[1]])));
            }
        }
        result
    }

    /// `refinementFeatures::findNearestEdge` for one sample (`indexedOctree::findNearest`
    /// of upstream, so ties resolve as in ESI). `ties` is unused (kept for the brute-force
    /// twins).
    pub fn find_nearest_edge(&self, sample: V, dist_sqr: f64, _ties: &mut TieCounts) -> EdgeHit {
        self.nearest_edge_tree(&self.edge_trees, sample, dist_sqr)
    }

    /// `refinementFeatures::findNearestRegionEdge` for one sample.
    pub fn find_nearest_region_edge(&self, sample: V, dist_sqr: f64, _ties: &mut TieCounts) -> EdgeHit {
        self.nearest_edge_tree(&self.region_edge_trees, sample, dist_sqr)
    }

    fn nearest_edge_tree(&self, trees: &[Octree<EdgeShapes>], sample: V, dist_sqr: f64) -> EdgeHit {
        let mut result: EdgeHit = None;
        for (feati, tree) in trees.iter().enumerate() {
            if tree.shapes().len() == 0 {
                continue;
            }
            let start = match &result {
                Some((_, _, pt, _)) => mag_sqr(sub(*pt, sample)),
                None => dist_sqr,
            };
            if let Some((shape, pt)) = tree.find_nearest(sample, start) {
                let ei = tree.shapes().ids[shape];
                let e = self.features[feati].edges[ei];
                let em = &self.features[feati];
                result = Some((feati, ei, pt, unit_vec(em.points[e[0]], em.points[e[1]])));
            }
        }
        result
    }

    /// Brute-force `findNearestEdge` (first minimum in index order), the round-6 search.
    pub fn find_nearest_edge_brute(&self, sample: V, dist_sqr: f64, ties: &mut TieCounts) -> EdgeHit {
        self.nearest_edge_brute(false, sample, dist_sqr, ties)
    }

    /// Brute-force `findNearestRegionEdge`.
    pub fn find_nearest_region_edge_brute(&self, sample: V, dist_sqr: f64, ties: &mut TieCounts) -> EdgeHit {
        self.nearest_edge_brute(true, sample, dist_sqr, ties)
    }

    /// `refinementFeatures::findNearestPoint` for one sample: feature index, feature point
    /// index, point.
    pub fn find_nearest_point(&self, sample: V, dist_sqr: f64, ties: &mut TieCounts) -> Option<(usize, usize, V)> {
        let mut result: Option<(usize, usize, V)> = None;
        for (feati, em) in self.features.iter().enumerate() {
            if em.non_feature_start == 0 {
                continue;
            }
            let start = match &result {
                Some((_, _, pt)) => mag_sqr(sub(*pt, sample)),
                None => dist_sqr,
            };
            let mut best = start;
            let mut hit: Option<usize> = None;
            let mut tied = false;
            for pi in 0..em.non_feature_start {
                let d2 = mag_sqr(sub(em.points[pi], sample));
                if d2 < best {
                    best = d2;
                    hit = Some(pi);
                    tied = false;
                } else if hit.is_some() && d2 == best {
                    tied = true;
                }
            }
            if tied {
                ties.feature_point += 1;
            }
            if let Some(pi) = hit {
                result = Some((feati, pi, em.points[pi]));
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------------------
// per-point surface data
// ---------------------------------------------------------------------------------------

/// Per patch point, per surrounding face (sorted by distance of the face centre):
/// surface normal, displacement to the surface, face centre, patch id.
#[derive(Debug, Clone, Default)]
pub struct PointFaceData {
    /// `pointFaceSurfNormals`.
    pub normals: Vec<Vec<V>>,
    /// `pointFaceDisp`.
    pub disp: Vec<Vec<V>>,
    /// `pointFaceCentres`.
    pub centres: Vec<Vec<V>>,
    /// `pointFacePatchID` (the STL region of the nearest triangle).
    pub patch_id: Vec<Vec<usize>>,
}

/// `calcNearestFace` (unzoned surfaces only) followed by `calcNearestFacePointProperties`
/// (serial; every boundary face is a patch face, every patch edge has two faces and there
/// are no duplicate faces, which is checked).
///
/// # Errors
/// [`FeatureError::NotPorted`] when a patch edge does not have exactly two faces.
pub fn calc_nearest_face_point_properties(
    pa: &PatchAddr,
    topo: &PatchTopo,
    local_points: &[V],
    snap_dist: &[f64],
    surface: &FeatureSurface,
    ties: &mut TieCounts,
) -> Result<PointFaceData, FeatureError> {
    if topo.edge_faces.iter().any(|ef| ef.len() != 2) {
        return Err(FeatureError::NotPorted("patch edge with other than two faces (boundary-edge points)".into()));
    }
    let n_faces = pa.local_faces.len();
    let centres: Vec<V> = pa.local_faces.iter().map(|f| face_centre(local_points, f)).collect();
    let mut face_disp = vec![[0.0; 3]; n_faces];
    let mut face_normal = vec![[0.0; 3]; n_faces];
    let mut face_region: Vec<Option<usize>> = vec![None; n_faces];
    for (fi, f) in pa.local_faces.iter().enumerate() {
        let face_snap_dist = f.iter().fold(-GREAT, |m, &p| m.max(snap_dist[p]));
        if let Some((ti, q, n)) = surface.nearest(centres[fi], face_snap_dist * face_snap_dist, ties) {
            face_disp[fi] = sub(q, centres[fi]);
            face_normal[fi] = n;
            face_region[fi] = Some(surface.region[ti]);
        }
    }
    let n_points = pa.mesh_points.len();
    let mut out = PointFaceData {
        normals: vec![Vec::new(); n_points],
        disp: vec![Vec::new(); n_points],
        centres: vec![Vec::new(); n_points],
        patch_id: vec![Vec::new(); n_points],
    };
    for pi in 0..n_points {
        let pt = local_points[pi];
        let mut rows: Vec<(V, V, V, usize)> = Vec::new();
        for &fi in &pa.point_faces[pi] {
            if let Some(region) = face_region[fi] {
                // `pFc[n] = centre`; later `p -= pt` and `p += pt` (a round trip that moves bits).
                let fc = add(sub(centres[fi], pt), pt);
                rows.push((face_normal[fi], face_disp[fi], fc, region));
            }
        }
        // sortedOrder(mag(pFc)): stable ascending sort.
        let mut order: Vec<usize> = (0..rows.len()).collect();
        let keys: Vec<f64> = rows.iter().map(|r| mag(r.2)).collect();
        order.sort_by(|&a, &b| keys[a].partial_cmp(&keys[b]).unwrap_or(std::cmp::Ordering::Equal));
        for &k in &order {
            out.normals[pi].push(rows[k].0);
            out.disp[pi].push(rows[k].1);
            out.centres[pi].push(rows[k].2);
            out.patch_id[pi].push(rows[k].3);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// per-point reconstruction
// ---------------------------------------------------------------------------------------

fn find_normal(feature_cos: f64, n: V, surface_normals: &[V]) -> Option<usize> {
    for (j, sn) in surface_normals.iter().enumerate() {
        let cos_angle = dot(n, *sn);
        if cos_angle >= feature_cos || cos_angle < (-1.0 + 0.001) {
            return Some(j);
        }
    }
    None
}

/// What the per-point reconstruction found.
#[derive(Debug, Clone)]
pub struct Reconstruction {
    /// Attraction vector.
    pub attraction: V,
    /// Constraint (1 plane, 2 edge, 3 point, 0 none).
    pub constraint: PointConstraint,
    /// Surface points collected.
    pub surface_points: Vec<V>,
    /// Surface normals collected.
    pub surface_normals: Vec<V>,
    /// Per surrounding face, the index of its normal bin.
    pub face_to_normal_bin: Vec<i64>,
}

fn limit(d: V, snap_dist: f64) -> V {
    if mag_sqr(d) > snap_dist * snap_dist {
        let s = (snap_dist * snap_dist / mag_sqr(d)).sqrt();
        scale(s, d)
    } else {
        d
    }
}

/// `featureAttractionUsingReconstruction` for one point (v2506 `:980`).
pub fn reconstruct_point(feature_cos: f64, pt: V, snap_dist: f64, pfd: &PointFaceData, pointi: usize) -> Reconstruction {
    let pf_normals = &pfd.normals[pointi];
    let pf_disp = &pfd.disp[pointi];
    let pf_centres = &pfd.centres[pointi];
    let mut surface_points: Vec<V> = Vec::new();
    let mut surface_normals: Vec<V> = Vec::new();
    let mut bin: Vec<i64> = vec![-1; pf_disp.len()];
    for i in 0..pf_normals.len() {
        let fc = pf_centres[i];
        let f_s_normal = pf_normals[i];
        let f_disp = pf_disp[i];
        if mag_sqr(f_disp) < snap_dist * snap_dist && mag(f_s_normal) > VSMALL {
            let p = add(fc, f_disp);
            bin[i] = find_normal(feature_cos, f_s_normal, &surface_normals).map(|j| j as i64).unwrap_or(-1);
            if bin[i] == -1 {
                if surface_points.len() <= 1 {
                    surface_points.push(p);
                    bin[i] = surface_normals.len() as i64;
                    surface_normals.push(f_s_normal);
                } else if surface_points.len() == 2 {
                    let pl0 = Plane::new(surface_points[0], surface_normals[0]);
                    let pl1 = Plane::new(surface_points[1], surface_normals[1]);
                    let (_, dir) = pl0.plane_intersect(&pl1);
                    let feature_normal = div(dir, mag(dir));
                    if mag(vdot(f_s_normal, feature_normal)) >= 0.001 {
                        surface_points.push(p);
                        bin[i] = surface_normals.len() as i64;
                        surface_normals.push(f_s_normal);
                    }
                } else if surface_points.len() == 3 {
                    let pl0 = Plane::new(surface_points[0], surface_normals[0]);
                    let pl1 = Plane::new(surface_points[1], surface_normals[1]);
                    let pl2 = Plane::new(surface_points[2], surface_normals[2]);
                    let p012 = pl0.plane_plane_intersect(&pl1, &pl2);
                    let (_, dir) = pl0.plane_intersect(&pl1);
                    let feature_normal = div(dir, mag(dir));
                    if mag(vdot(f_s_normal, feature_normal)) >= 0.001 {
                        let pl3 = Plane::new(p, f_s_normal);
                        let p013 = pl0.plane_plane_intersect(&pl1, &pl3);
                        if mag(sub(p012, p013)) > snap_dist {
                            surface_points.push(p);
                            bin[i] = surface_normals.len() as i64;
                            surface_normals.push(f_s_normal);
                        }
                    }
                }
            }
        }
    }
    let mut attraction = [0.0; 3];
    let mut constraint = PointConstraint::none();
    if surface_normals.len() == 1 {
        let d = scale(dot(sub(surface_points[0], pt), surface_normals[0]), surface_normals[0]);
        attraction = limit(d, snap_dist);
        constraint.apply_constraint(surface_normals[0]);
    } else if surface_normals.len() == 2 {
        let pl0 = Plane::new(surface_points[0], surface_normals[0]);
        let pl1 = Plane::new(surface_points[1], surface_normals[1]);
        let (ref_point, dir) = pl0.plane_intersect(&pl1);
        let n = div(dir, mag(dir));
        let mut d = sub(ref_point, pt);
        d = sub(d, scale(dot(d, n), n));
        attraction = limit(d, snap_dist);
        constraint.apply_constraint(surface_normals[0]);
        constraint.apply_constraint(surface_normals[1]);
    } else if surface_normals.len() == 3 {
        let pl0 = Plane::new(surface_points[0], surface_normals[0]);
        let pl1 = Plane::new(surface_points[1], surface_normals[1]);
        let pl2 = Plane::new(surface_points[2], surface_normals[2]);
        let corner = pl0.plane_plane_intersect(&pl1, &pl2);
        let d = sub(corner, pt);
        attraction = limit(d, snap_dist);
        constraint.apply_constraint(surface_normals[0]);
        constraint.apply_constraint(surface_normals[1]);
        constraint.apply_constraint(surface_normals[2]);
    }
    Reconstruction { attraction, constraint, surface_points, surface_normals, face_to_normal_bin: bin }
}

// ---------------------------------------------------------------------------------------
// determineFeatures
// ---------------------------------------------------------------------------------------

/// Mutable state of one `featureAttractionUsingFeatureEdges` call.
#[derive(Debug, Clone)]
pub struct AttractState {
    /// `edgeAttractors[feati][edge]`: feature-edge points mesh points were attracted to.
    pub edge_attractors: Vec<Vec<Vec<V>>>,
    /// `edgeConstraints[feati][edge]`.
    pub edge_constraints: Vec<Vec<Vec<PointConstraint>>>,
    /// `pointAttractor[feati][featPoint]`: attracted patch point or -1.
    pub point_attractor: Vec<Vec<i64>>,
    /// `pointConstraints[feati][featPoint]`.
    pub point_constraints: Vec<Vec<PointConstraint>>,
    /// `rawPatchAttraction` / `patchAttraction`.
    pub patch_attraction: Vec<V>,
    /// `rawPatchConstraints` / `patchConstraints`.
    pub patch_constraints: Vec<PointConstraint>,
}

impl AttractState {
    fn new(fs: &FeatureSet, n_points: usize) -> Self {
        Self {
            edge_attractors: fs.features.iter().map(|f| vec![Vec::new(); f.edges.len()]).collect(),
            edge_constraints: fs.features.iter().map(|f| vec![Vec::new(); f.edges.len()]).collect(),
            point_attractor: fs.features.iter().map(|f| vec![-1; f.points.len()]).collect(),
            point_constraints: fs.features.iter().map(|f| vec![PointConstraint::none(); f.points.len()]).collect(),
            patch_attraction: vec![[0.0; 3]; n_points],
            patch_constraints: vec![PointConstraint::none(); n_points],
        }
    }
}

/// `findNearFeatureEdge`: returns the feature index (or -1 when nothing is in range) and
/// whether the edge search hit.
fn find_near_feature_edge(
    fs: &FeatureSet,
    is_region_edge: bool,
    local_points: &[V],
    snap_dist: &[f64],
    pointi: usize,
    estimated: V,
    st: &mut AttractState,
    ties: &mut TieCounts,
) -> (i64, bool) {
    let d2 = snap_dist[pointi] * snap_dist[pointi];
    let hit = if is_region_edge {
        fs.find_nearest_region_edge(estimated, d2, ties)
    } else {
        fs.find_nearest_edge(estimated, d2, ties)
    };
    match hit {
        Some((feati, ei, pt, normal)) => {
            st.edge_attractors[feati][ei].push(pt);
            let c = PointConstraint { n: 2, dir: normal };
            st.edge_constraints[feati][ei].push(c);
            st.patch_attraction[pointi] = sub(pt, local_points[pointi]);
            st.patch_constraints[pointi] = c;
            (feati as i64, true)
        }
        None => (-1, false),
    }
}

/// `findNearFeaturePoint`: returns the feature index after the attractor logic (-1 when
/// the point lost to a nearer patch point) and whether the point search hit.
fn find_near_feature_point(
    fs: &FeatureSet,
    is_region_point: bool,
    local_points: &[V],
    snap_dist: &[f64],
    pointi: usize,
    estimated: V,
    st: &mut AttractState,
    ties: &mut TieCounts,
) -> (i64, bool) {
    let d2 = snap_dist[pointi] * snap_dist[pointi];
    let Some((mut feati, feat_pointi, feat_pt)) = fs.find_nearest_point(estimated, d2, ties).map(|(f, i, p)| (f as i64, i, p)) else {
        return (-1, false);
    };
    let pt = local_points[pointi];
    let dist_sqr = mag_sqr(sub(feat_pt, pt));
    let old = st.point_attractor[feati as usize][feat_pointi];
    if old != -1 {
        let old = old as usize;
        if dist_sqr >= mag_sqr(sub(feat_pt, local_points[old])) {
            feati = -1;
        } else {
            let f = feati as usize;
            st.point_attractor[f][feat_pointi] = pointi as i64;
            st.point_constraints[f][feat_pointi] = PointConstraint { n: 3, dir: [0.0; 3] };
            st.patch_attraction[pointi] = sub(feat_pt, pt);
            st.patch_constraints[pointi] = st.point_constraints[f][feat_pointi];
            st.patch_attraction[old] = [0.0; 3];
            st.patch_constraints[old] = PointConstraint::none();
            find_near_feature_edge(fs, is_region_point, local_points, snap_dist, old, local_points[old], st, ties);
        }
    } else {
        let f = feati as usize;
        st.point_attractor[f][feat_pointi] = pointi as i64;
        st.point_constraints[f][feat_pointi] = PointConstraint { n: 3, dir: [0.0; 3] };
        st.patch_attraction[pointi] = sub(feat_pt, pt);
        st.patch_constraints[pointi] = st.point_constraints[f][feat_pointi];
    }
    (feati, true)
}

/// `findMultiPatchPoint` (second overload): `(hit, index)`; index 1 means a point where
/// two patches meet over different normals, 0 an edge.
fn find_multi_patch_point(patch_ids: &[usize], surface_normals: &[V], face_to_normal_bin: &[i64]) -> (bool, i32) {
    if patch_ids.is_empty() {
        return (false, -1);
    }
    let patch0 = patch_ids[0];
    if patch_ids.iter().skip(1).all(|&p| p == patch0) {
        return (false, -1);
    }
    if surface_normals.len() == 1 {
        return (true, 1);
    }
    let mut normal_to_patch: Vec<i64> = vec![-1; surface_normals.len()];
    for (i, &b) in face_to_normal_bin.iter().enumerate() {
        if b != -1 {
            let patch = &mut normal_to_patch[b as usize];
            if *patch == -1 {
                *patch = patch_ids[i] as i64;
            } else if *patch == -2 {
            } else if *patch != patch_ids[i] as i64 {
                *patch = -2;
            }
        }
    }
    if normal_to_patch.iter().any(|&p| p == -2) {
        return (true, 1);
    }
    (true, 0)
}

/// `determineFeatures` (v2506 `:2424`), `strictRegionFeatureSnap` false.
#[allow(clippy::too_many_arguments)]
pub fn determine_features(
    fs: &FeatureSet,
    feature_cos: f64,
    multi_region: bool,
    local_points: &[V],
    snap_dist: &[f64],
    nearest_disp: &[V],
    pfd: &PointFaceData,
    st: &mut AttractState,
    ties: &mut TieCounts,
) {
    for pointi in 0..local_points.len() {
        let pt = local_points[pointi];
        let rec = reconstruct_point(feature_cos, pt, snap_dist[pointi], pfd, pointi);
        let better = rec.constraint.n > st.patch_constraints[pointi].n
            || (rec.constraint.n == st.patch_constraints[pointi].n
                && mag_sqr(rec.attraction) < mag_sqr(st.patch_attraction[pointi]));
        if !better {
            continue;
        }
        st.patch_attraction[pointi] = rec.attraction;
        st.patch_constraints[pointi] = rec.constraint;
        let n = st.patch_constraints[pointi].n;
        if n == 1 {
            if multi_region {
                let estimated = add(pt, nearest_disp[pointi]);
                let (hit, _) = find_multi_patch_point(&pfd.patch_id[pointi], &rec.surface_normals, &rec.face_to_normal_bin);
                if hit {
                    find_near_feature_edge(fs, true, local_points, snap_dist, pointi, estimated, st, ties);
                }
            }
        } else if n == 2 {
            let estimated = add(pt, st.patch_attraction[pointi]);
            let mut has_snapped = false;
            if multi_region {
                let (hit, index) = find_multi_patch_point(&pfd.patch_id[pointi], &rec.surface_normals, &rec.face_to_normal_bin);
                if hit {
                    if index == 0 {
                        find_near_feature_edge(fs, true, local_points, snap_dist, pointi, estimated, st, ties);
                        has_snapped = true;
                    } else {
                        let (_, point_hit) = find_near_feature_point(fs, true, local_points, snap_dist, pointi, estimated, st, ties);
                        has_snapped = true;
                        if !point_hit {
                            find_near_feature_edge(fs, true, local_points, snap_dist, pointi, estimated, st, ties);
                        }
                    }
                }
            }
            if !has_snapped {
                find_near_feature_edge(fs, false, local_points, snap_dist, pointi, estimated, st, ties);
            }
        } else if n == 3 {
            let estimated = add(pt, st.patch_attraction[pointi]);
            let mut region_point = false;
            if multi_region {
                let (hit, _) = find_multi_patch_point(&pfd.patch_id[pointi], &rec.surface_normals, &rec.face_to_normal_bin);
                region_point = hit;
            }
            // Upstream passes `true` (region point) when a multi-patch point is found; the
            // flag only reaches the edge search of a displaced old attractor.
            find_near_feature_point(fs, region_point, local_points, snap_dist, pointi, estimated, st, ties);
        }
    }
}

// ---------------------------------------------------------------------------------------
// baffles, reverse attraction, stringing, squeeze
// ---------------------------------------------------------------------------------------

/// The number of baffle edges (patch edges with two faces whose unit normals have a dot
/// product below `baffleFeatureCos`): the detection half of `determineBaffleFeatures`, kept as
/// a cross-check of [`determine_baffle_features`] (which also attracts the end points).
pub fn count_baffle_edges(pa: &PatchAddr, topo: &PatchTopo, local_points: &[V], baffle_feature_cos: f64) -> usize {
    let normals: Vec<V> = pa
        .local_faces
        .iter()
        .map(|f| {
            let n = esi_face_area_normal(local_points, f);
            let s = mag(n);
            if s < ROOTVSMALL {
                [0.0; 3]
            } else {
                div(n, s)
            }
        })
        .collect();
    topo.edge_faces
        .iter()
        .filter(|ef| ef.len() == 2 && dot(normals[ef[0]], normals[ef[1]]) < baffle_feature_cos)
        .count()
}

/// What `determineBaffleFeatures` printed ("Baffle points : N of which attracted to").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BaffleStats {
    /// "Detected N baffle edges".
    pub edges: usize,
    /// "Baffle points : N" (end points of baffle edges).
    pub points: usize,
    /// "feature point".
    pub feature_point: usize,
    /// "feature edge".
    pub feature_edge: usize,
}

/// `determineBaffleFeatures` (v2506 `:2990`) with `baffleFeaturePoints false` (the default;
/// the feature-point loop and the `pointStatus == 1` branch are disabled upstream:
/// "TEMPORARILY DISABLED"). Every end point of a baffle edge (patch edge with two faces whose
/// unit normals have a dot product below `baffleFeatureCos`) is attracted to its nearest
/// feature edge, overwriting whatever the geometric analysis stored for it, in point order.
///
/// # Errors
/// [`FeatureError::NotPorted`] when `baffle_feature_points` is true (not ported).
pub fn determine_baffle_features(
    fs: &FeatureSet,
    pa: &PatchAddr,
    topo: &PatchTopo,
    local_points: &[V],
    snap_dist: &[f64],
    baffle_feature_cos: f64,
    baffle_feature_points: bool,
    st: &mut AttractState,
    ties: &mut TieCounts,
) -> Result<BaffleStats, FeatureError> {
    if baffle_feature_points {
        return Err(FeatureError::NotPorted("baffleFeaturePoints true".into()));
    }
    let normals: Vec<V> = pa
        .local_faces
        .iter()
        .map(|f| {
            let n = esi_face_area_normal(local_points, f);
            let s = mag(n);
            if s < ROOTVSMALL {
                [0.0; 3]
            } else {
                div(n, s)
            }
        })
        .collect();
    let mut status = vec![-1i32; local_points.len()];
    let mut stats = BaffleStats::default();
    for (ei, ef) in topo.edge_faces.iter().enumerate() {
        if ef.len() == 2 && dot(normals[ef[0]], normals[ef[1]]) < baffle_feature_cos {
            stats.edges += 1;
            let e = topo.edges[ei];
            status[e[0]] = 0;
            status[e[1]] = 0;
        }
    }
    stats.points = status.iter().filter(|&&s| s != -1).count();
    for pointi in 0..local_points.len() {
        if status[pointi] == 0 {
            let pt = local_points[pointi];
            let (_, hit) = find_near_feature_edge(fs, false, local_points, snap_dist, pointi, pt, st, ties);
            if hit {
                stats.feature_edge += 1;
            }
        }
    }
    Ok(stats)
}

/// Counts of `writeStats`: points attracted to a feature point / feature edge / the
/// nearest surface / none of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttractStats {
    /// Master points.
    pub total: usize,
    /// Constraint 3.
    pub feature_point: usize,
    /// Constraint 2.
    pub feature_edge: usize,
    /// Constraint 1.
    pub nearest_surface: usize,
    /// The rest.
    pub rest: usize,
}

/// `writeStats` (serial: every point is a master point).
pub fn attract_stats(constraints: &[PointConstraint]) -> AttractStats {
    let c = |n: i32| constraints.iter().filter(|x| x.n == n).count();
    let (p, e, s) = (c(3), c(2), c(1));
    AttractStats { total: constraints.len(), feature_point: p, feature_edge: e, nearest_surface: s, rest: constraints.len() - p - e - s }
}

/// `reverseAttractMeshPoints` (v2506 `:3330`): attract the mesh point nearest to each
/// feature-edge / feature-point attractor instead of the other way round. Returns the
/// number of points initially selected and after one grow, and the new
/// attraction / constraints.
#[allow(clippy::too_many_arguments)]
pub fn reverse_attract_mesh_points(
    fs: &FeatureSet,
    pa: &PatchAddr,
    local_points: &[V],
    st: &AttractState,
    ties: &mut TieCounts,
) -> (usize, usize, Vec<V>, Vec<PointConstraint>) {
    let n = local_points.len();
    let mut is_feat: Vec<bool> = st.patch_constraints.iter().map(|c| c.n >= 2).collect();
    let n_init = is_feat.iter().filter(|&&b| b).count();
    let mut grown = is_feat.clone();
    for f in &pa.local_faces {
        if f.iter().any(|&p| is_feat[p]) {
            for &p in f {
                grown[p] = true;
            }
        }
    }
    is_feat = grown;
    let attract_points: Vec<usize> = (0..n).filter(|&i| is_feat[i]).collect();
    let nearest_pp = |feat_pt: V, ties: &mut TieCounts| -> Option<usize> {
        let mut best = GREAT * GREAT;
        let mut hit = None;
        let mut tied = false;
        for &pi in &attract_points {
            let d2 = mag_sqr(sub(local_points[pi], feat_pt));
            if d2 < best {
                best = d2;
                hit = Some(pi);
                tied = false;
            } else if hit.is_some() && d2 == best {
                tied = true;
            }
        }
        if tied {
            ties.patch_point += 1;
        }
        hit
    };
    let mut patch_attraction = vec![[0.0; 3]; n];
    let mut patch_constraints = vec![PointConstraint::none(); n];
    for (feati, edge_attr) in st.edge_attractors.iter().enumerate() {
        for (ei, attr) in edge_attr.iter().enumerate() {
            for (i, feat_pt) in attr.iter().enumerate() {
                if let Some(pointi) = nearest_pp(*feat_pt, ties) {
                    let attraction = sub(*feat_pt, local_points[pointi]);
                    if patch_constraints[pointi].n <= 1 || mag_sqr(attraction) < mag_sqr(patch_attraction[pointi]) {
                        patch_attraction[pointi] = attraction;
                        patch_constraints[pointi] = st.edge_constraints[feati][ei][i];
                    }
                }
            }
        }
    }
    for (feati, point_attr) in st.point_attractor.iter().enumerate() {
        for (fpi, &attractor) in point_attr.iter().enumerate() {
            if attractor != -1 {
                let feat_pt = fs.features[feati].points[fpi];
                if let Some(pointi) = nearest_pp(feat_pt, ties) {
                    let attraction = sub(feat_pt, local_points[pointi]);
                    let c = patch_constraints[pointi].n;
                    if c <= 1 || c == 2 || (c == 3 && mag_sqr(attraction) < mag_sqr(patch_attraction[pointi])) {
                        patch_attraction[pointi] = attraction;
                        patch_constraints[pointi] = st.point_constraints[feati][fpi];
                    }
                }
            }
        }
    }
    (n_init, attract_points.len(), patch_attraction, patch_constraints)
}

/// `stringFeatureEdges` (v2506 `:1284`). Returns the number of points changed per pass.
/// (Upstream reads `rawPatchConstraints[pointi]`, not `[nbrPointi]`, for `nbrFeatVec`;
/// kept as written.)
pub fn string_feature_edges(
    feature_cos: f64,
    topo: &PatchTopo,
    local_points: &[V],
    raw_attraction: &[V],
    raw_constraints: &[PointConstraint],
    patch_attraction: &mut [V],
    patch_constraints: &mut [PointConstraint],
) -> Vec<usize> {
    let mut passes = Vec::new();
    loop {
        let mut n_changed = 0;
        for pointi in 0..local_points.len() {
            if patch_constraints[pointi].n != 2 {
                continue;
            }
            let pt = local_points[pointi];
            let p_edges = &topo.point_edges[pointi];
            let feat_vec = patch_constraints[pointi].dir;
            let mut has_pos = false;
            let mut has_neg = false;
            let other = |ei: usize| {
                let e = topo.edges[ei];
                if e[0] == pointi {
                    e[1]
                } else {
                    e[0]
                }
            };
            for &ei in p_edges {
                let nbr = other(ei);
                if patch_constraints[nbr].n > 1 {
                    let feat_pt = add(local_points[nbr], patch_attraction[nbr]);
                    if dot(feat_vec, sub(feat_pt, pt)) > 0.0 {
                        has_pos = true;
                    } else {
                        has_neg = true;
                    }
                }
            }
            if !has_pos || !has_neg {
                let mut best_pos: Option<usize> = None;
                let mut min_pos = GREAT;
                let mut best_neg: Option<usize> = None;
                let mut min_neg = GREAT;
                for &ei in p_edges {
                    let nbr = other(ei);
                    if patch_constraints[nbr].n <= 1 && raw_constraints[nbr].n > 1 {
                        let nbr_feat_vec = raw_constraints[pointi].dir;
                        if dot(feat_vec, nbr_feat_vec).abs() > feature_cos {
                            let d2 = mag_sqr(raw_attraction[nbr]);
                            let feat_pt = add(local_points[nbr], raw_attraction[nbr]);
                            if dot(feat_vec, sub(feat_pt, pt)) > 0.0 {
                                if !has_pos && d2 < min_pos {
                                    min_pos = d2;
                                    best_pos = Some(nbr);
                                }
                            } else if !has_neg && d2 < min_neg {
                                min_neg = d2;
                                best_neg = Some(nbr);
                            }
                        }
                    }
                }
                for b in [best_pos, best_neg].into_iter().flatten() {
                    patch_attraction[b] = scale(0.5, raw_attraction[b]);
                    patch_constraints[b] = raw_constraints[b];
                    n_changed += 1;
                }
            }
        }
        passes.push(n_changed);
        if n_changed == 0 {
            break;
        }
    }
    passes
}

/// `preventFaceSqueeze` (v2506 `:3915`).
pub fn prevent_face_squeeze(
    pa: &PatchAddr,
    local_points: &[V],
    nearest_attraction: &[V],
    patch_attraction: &mut [V],
    patch_constraints: &[PointConstraint],
) {
    for f in &pa.local_faces {
        let mut points: Vec<V> = Vec::with_capacity(f.len());
        let mut n_constraints = 0;
        for &pointi in f {
            let pt = local_points[pointi];
            if patch_constraints[pointi].n > 1 {
                points.push(add(pt, patch_attraction[pointi]));
                n_constraints += 1;
            } else {
                points.push(pt);
            }
        }
        if n_constraints != f.len() {
            continue;
        }
        if f.len() == 3 {
            let mut max_fp: Option<usize> = None;
            let mut max_s = -1.0;
            for fp in 0..3 {
                let s = mag_sqr(sub(local_points[f[(fp + 1) % 3]], local_points[f[fp]]));
                if s > max_s {
                    max_s = s;
                    max_fp = Some(fp);
                }
            }
            if let Some(fp) = max_fp {
                let pointi = f[(fp + 2) % 3];
                patch_attraction[pointi] = nearest_attraction[pointi];
            }
        } else {
            let old_area = mag(esi_face_area_normal(local_points, f));
            let single: Vec<usize> = (0..f.len()).collect();
            let new_area = mag(esi_face_area_normal(&points, &single));
            if new_area < 0.1 * old_area {
                let mut max_fp: Option<usize> = None;
                let mut max_s = -1.0;
                for (fp, &p) in f.iter().enumerate() {
                    let s = mag_sqr(patch_attraction[p]);
                    if s > max_s {
                        max_s = s;
                        max_fp = Some(fp);
                    }
                }
                if let Some(fp) = max_fp {
                    patch_attraction[f[fp]] = scale(0.5, patch_attraction[f[fp]]);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// calcNearestSurfaceFeature
// ---------------------------------------------------------------------------------------

/// `PatchTools::pointNormals` (serial): sum of the unit face normals of the faces at
/// each point (`face::unitNormal`), divided by `mag + VSMALL`.
pub fn patch_point_normals(pa: &PatchAddr, local_points: &[V]) -> Vec<V> {
    let normals: Vec<V> = pa
        .local_faces
        .iter()
        .map(|f| {
            let n = esi_face_area_normal(local_points, f);
            let s = mag(n);
            if s < ROOTVSMALL {
                [0.0; 3]
            } else {
                div(n, s)
            }
        })
        .collect();
    let mut extrude = vec![[0.0; 3]; local_points.len()];
    for (fi, f) in pa.local_faces.iter().enumerate() {
        for &p in f {
            extrude[p] = add(extrude[p], normals[fi]);
        }
    }
    extrude.iter().map(|&n| div(n, mag(n) + VSMALL)).collect()
}

/// `smoothAndConstrain` (v2506 `:132`), 20 averaging passes.
pub fn smooth_and_constrain(topo: &PatchTopo, constraints: &[PointConstraint], disp: &mut [V]) {
    for _ in 0..20 {
        let mut sum = vec![[0.0; 3]; disp.len()];
        let mut count = vec![0usize; disp.len()];
        for pointi in 0..disp.len() {
            let nc = constraints[pointi].n;
            if nc <= 1 {
                for &ei in &topo.point_edges[pointi] {
                    let e = topo.edges[ei];
                    let nbr = if e[0] == pointi { e[1] } else { e[0] };
                    if constraints[nbr].n >= nc {
                        sum[pointi] = add(sum[pointi], disp[nbr]);
                        count[pointi] += 1;
                    }
                }
            }
        }
        for pointi in 0..disp.len() {
            if count[pointi] > 0 {
                disp[pointi] = scale(0.5, add(disp[pointi], div(sum[pointi], count[pointi] as f64)));
            }
        }
    }
}

/// Controls of the feature attraction (`snapParameters`).
#[derive(Debug, Clone, Copy)]
pub struct FeatureControls {
    /// `featureCos = cos(degToRad(resolveFeatureAngle))`.
    pub feature_cos: f64,
    /// `cos(degToRad(110))`, the baffle-edge threshold.
    pub baffle_feature_cos: f64,
    /// `multiRegionFeatureSnap`.
    pub multi_region: bool,
    /// `stringFeatures`.
    pub string_features: bool,
}

/// What one `calcNearestSurfaceFeature` call printed (the gates of the V&V record).
#[derive(Debug, Clone)]
pub struct FeatureReport {
    /// "Raw geometric feature analysis".
    pub raw: AttractStats,
    /// "Detected N baffle edges".
    pub baffle_edges: usize,
    /// "Baffle points : N of which attracted to" (round 7).
    pub baffle: BaffleStats,
    /// "After baffle feature analysis" (round 7).
    pub after_baffle: AttractStats,
    /// "Initially selected N mesh points ... for reverse attraction".
    pub reverse_initial: usize,
    /// "Selected N mesh points".
    pub reverse_grown: usize,
    /// "Reverse attract feature analysis".
    pub reverse: AttractStats,
    /// "Stringing feature edges : changed N points", per pass.
    pub stringing: Vec<usize>,
    /// `Attraction: linear` max (vector of largest magSqr) and average.
    pub linear: (V, V),
    /// `Attraction: feature` max and average.
    pub feature: (V, V),
    /// "Feature analysis".
    pub fin: AttractStats,
    /// Tie counts of the searches.
    pub ties: TieCounts,
    /// Final per-point constraints (after stringing and squeeze prevention).
    pub constraints: Vec<PointConstraint>,
    /// Final per-point feature attraction.
    pub attraction: Vec<V>,
    /// The returned patch displacement (blended, smoothed, relaxed).
    pub patch_disp: Vec<V>,
}

fn g_max_mag_sqr(f: &[V]) -> V {
    let mut m = f[0];
    for &x in f {
        if mag_sqr(x) > mag_sqr(m) {
            m = x;
        }
    }
    m
}

fn g_average(f: &[V]) -> V {
    let mut s = [0.0; 3];
    for &x in f {
        s = add(s, x);
    }
    div(s, f.len() as f64)
}

/// `calcNearestSurfaceFeature` for explicit features (v2506 `:4049`): the patch
/// displacement of one morph iteration, from the nearest-surface displacement
/// `nearest_disp` of the same iteration.
///
/// # Errors
/// [`FeatureError::NotPorted`] for baffle edges or open patch edges.
#[allow(clippy::too_many_arguments)]
pub fn calc_nearest_surface_feature(
    ctl: &FeatureControls,
    fs: &FeatureSet,
    surface: &FeatureSurface,
    pa: &PatchAddr,
    topo: &PatchTopo,
    local_points: &[V],
    snap_dist: &[f64],
    nearest_disp: &[V],
    feature_attract: f64,
) -> Result<(Vec<V>, FeatureReport), FeatureError> {
    let mut ties = TieCounts::default();
    let pfd = calc_nearest_face_point_properties(pa, topo, local_points, snap_dist, surface, &mut ties)?;
    let mut patch_disp = nearest_disp.to_vec();
    let n = local_points.len();
    let mut raw = AttractState::new(fs, n);
    determine_features(fs, ctl.feature_cos, ctl.multi_region, local_points, snap_dist, nearest_disp, &pfd, &mut raw, &mut ties);
    let raw_stats = attract_stats(&raw.patch_constraints);
    let baffle = determine_baffle_features(fs, pa, topo, local_points, snap_dist, ctl.baffle_feature_cos, false, &mut raw, &mut ties)?;
    let baffle_edges = baffle.edges;
    let after_baffle = attract_stats(&raw.patch_constraints);
    let (reverse_initial, reverse_grown, mut patch_attraction, mut patch_constraints) =
        reverse_attract_mesh_points(fs, pa, local_points, &raw, &mut ties);
    let reverse = attract_stats(&patch_constraints);
    let stringing = if ctl.string_features {
        string_feature_edges(
            ctl.feature_cos,
            topo,
            local_points,
            &raw.patch_attraction,
            &raw.patch_constraints,
            &mut patch_attraction,
            &mut patch_constraints,
        )
    } else {
        Vec::new()
    };
prevent_face_squeeze(pa, local_points, nearest_disp, &mut patch_attraction, &patch_constraints);
    let linear = (g_max_mag_sqr(&patch_disp), g_average(&patch_disp));
    let feature = (g_max_mag_sqr(&patch_attraction), g_average(&patch_attraction));
    for pointi in 0..n {
        if patch_constraints[pointi].n > 1 {
            patch_disp[pointi] = add(scale(1.0 - feature_attract, patch_disp[pointi]), scale(feature_attract, patch_attraction[pointi]));
        }
    }
    let fin = attract_stats(&patch_constraints);
    if feature_attract < 1.0 - 0.001 {
        let point_normals = patch_point_normals(pa, local_points);
        let mut smoothed = patch_disp.clone();
        smooth_and_constrain(topo, &patch_constraints, &mut smoothed);
        let mut tang = patch_disp.clone();
        for i in 0..n {
            tang[i] = sub(tang[i], scale(dot(point_normals[i], patch_disp[i]), point_normals[i]));
        }
        smooth_and_constrain(topo, &patch_constraints, &mut tang);
        for i in 0..n {
            tang[i] = add(tang[i], scale(dot(point_normals[i], patch_disp[i]), point_normals[i]));
        }
        for i in 0..n {
            patch_disp[i] = add(scale(1.0 - feature_attract, smoothed[i]), scale(feature_attract, tang[i]));
        }
    }
    for d in patch_disp.iter_mut() {
        *d = scale(feature_attract, *d);
    }
    let report = FeatureReport {
        raw: raw_stats,
        baffle_edges,
        baffle,
        after_baffle,
        reverse_initial,
        reverse_grown,
        reverse,
        stringing,
        linear,
        feature,
        fin,
        ties,
        constraints: patch_constraints,
        attraction: patch_attraction,
        patch_disp: patch_disp.clone(),
    };
    Ok((patch_disp, report))
}

/// Everything a feature-snap morph iteration needs, fixed for the whole `doSnap` loop.
#[derive(Debug, Clone)]
pub struct FeatureSnap {
    /// The features.
    pub features: FeatureSet,
    /// The surface.
    pub surface: FeatureSurface,
    /// Patch addressing of the (fixed) topology.
    pub pa: PatchAddr,
    /// Patch edges.
    pub topo: PatchTopo,
    /// Mesh edges (for the displacement smoothing).
    pub mesh_edges: MeshEdges,
    /// Cell addressing (for `scaleMesh`).
    pub scale_topo: ScaleTopology,
    /// `calcSnapDistance` on the castellated mesh.
    pub snap_dist: Vec<f64>,
    /// Feature controls.
    pub controls: FeatureControls,
    /// `scaleMesh` controls.
    pub scale_controls: ScaleControls,
    /// `nFeatureSnapIter`.
    pub n_feature_iter: usize,
    /// `nSolveIter` (`smoothDisplacement` passes).
    pub n_solve_iter: usize,
    /// `nRelaxIter`.
    pub n_relax_iter: usize,
}

/// One morph iteration's results.
#[derive(Debug, Clone)]
pub struct MorphIteration {
    /// The log gates of `calcNearestSurfaceFeature`.
    pub report: FeatureReport,
    /// Mean, min and max of the wanted (nearest-surface) displacement magnitudes.
    pub wanted: (f64, f64, f64),
    /// The smoothed displacement on all mesh points (`N/displacement`).
    pub displacement: Vec<V>,
    /// The `scaleMesh` passes.
    pub scale_passes: Vec<ScaleIteration>,
}

impl FeatureSnap {
    /// One iteration of the `doSnap` morph loop (v2506 `snappySnapDriver.C:3305-3420`)
    /// without `detectNearSurfaces`, face splitting and frozen points: nearest surface,
    /// feature attraction with blend `(iter+1)/nFeatureSnapIter`, `setDisplacement`,
    /// `smoothDisplacement`, `scaleMesh`. Moves `mesh.points`.
    ///
    /// # Errors
    /// [`FeatureError::NotPorted`] from the feature attraction, or when a patch point has
    /// no surface within its snap distance.
    pub fn morph_iteration(&self, mesh: &mut SnapMesh, iter: usize) -> Result<MorphIteration, FeatureError> {
        self.morph_iteration_from(mesh, iter, None)
    }

    /// [`Self::morph_iteration`] with the displacement field the previous iteration left in
    /// `meshMover` (`prev`, all mesh points; `None` for the first iteration, where it is
    /// zero). Upstream's `motionSmoother::setDisplacement` writes only the patch points, so
    /// the internal points start the next `smoothDisplacement` from the previous smoothed
    /// values (round 7: starting them from zero left 1e-11 m of residual after 300 passes).
    ///
    /// # Errors
    /// As [`Self::morph_iteration`].
    pub fn morph_iteration_from(&self, mesh: &mut SnapMesh, iter: usize, prev: Option<&[V]>) -> Result<MorphIteration, FeatureError> {
        let local: Vec<V> = self.pa.mesh_points.iter().map(|&p| mesh.points[p]).collect();
        let mut ties = TieCounts::default();
        let (nearest, missing) = calc_nearest_surface_sqr_mag(&local, &self.snap_dist, &self.surface, &mut ties);
        if missing > 0 {
            return Err(FeatureError::NotPorted(format!("{missing} patch points with no surface in range")));
        }
        let mags: Vec<f64> = nearest.iter().map(|&d| mag(d)).collect();
        let wanted = (
            mags.iter().sum::<f64>() / mags.len() as f64,
            mags.iter().cloned().fold(f64::MAX, f64::min),
            mags.iter().cloned().fold(0.0, f64::max),
        );
        let attract = (iter + 1) as f64 / self.n_feature_iter as f64;
        let (patch_disp, mut report) = calc_nearest_surface_feature(
            &self.controls,
            &self.features,
            &self.surface,
            &self.pa,
            &self.topo,
            &local,
            &self.snap_dist,
            &nearest,
            attract,
        )?;
        report.ties.triangle += ties.triangle;
        let mut disp = match prev {
            Some(p) => {
                let mut d = p.to_vec();
                for (i, &mp) in self.pa.mesh_points.iter().enumerate() {
                    d[mp] = patch_disp[i];
                }
                d
            }
            None => displacement_field(mesh, &self.pa, &patch_disp),
        };
        smooth_displacement(mesh, &self.mesh_edges, &self.pa, &mut disp, self.n_solve_iter);
        let mut ms = MotionSmoother::new(mesh, &self.pa, disp.clone());
        let scale_passes =
            scale_mesh_loop(mesh, &mut ms, &self.pa, &self.mesh_edges, &self.scale_topo, &self.scale_controls, self.n_relax_iter);
        Ok(MorphIteration { report, wanted, displacement: disp, scale_passes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_intersections_of_the_coordinate_planes() {
        let x = Plane::new([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let y = Plane::new([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]);
        let z = Plane::new([0.0, 0.0, 3.0], [0.0, 0.0, 1.0]);
        let p = x.plane_plane_intersect(&y, &z);
        assert_eq!(p, [1.0, 2.0, 3.0]);
        let (pt, dir) = x.plane_intersect(&y);
        assert_eq!((pt[0], pt[1]), (1.0, 2.0));
        assert_eq!(dir, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn point_constraint_counts_directions() {
        let mut c = PointConstraint::none();
        c.apply_constraint([1.0, 0.0, 0.0]);
        assert_eq!(c.n, 1);
        c.apply_constraint([1.0, 0.0, 0.0]);
        assert_eq!(c.n, 1, "the same normal does not add a direction");
        c.apply_constraint([0.0, 1.0, 0.0]);
        assert_eq!((c.n, c.dir), (2, [0.0, 0.0, -1.0]), "(0,1,0) x (1,0,0)");
        c.apply_constraint([0.0, 0.0, 1.0]);
        assert_eq!(c.n, 3);
    }

    #[test]
    fn line_nearest_clamps_to_the_ends() {
        let (p, d) = line_nearest([0.0; 3], [1.0, 0.0, 0.0], [2.0, 1.0, 0.0]);
        assert_eq!(p, [1.0, 0.0, 0.0]);
        assert!((d * d - 2.0).abs() < 1e-14);
        let (p, _) = line_nearest([0.0; 3], [1.0, 0.0, 0.0], [0.5, 1.0, 0.0]);
        assert_eq!(p, [0.5, 0.0, 0.0]);
    }
}
