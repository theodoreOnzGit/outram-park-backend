// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/meshes/polyMeshGenChecks/{polyMeshGenChecks.C,
//     polyMeshGenChecks.H, polyMeshGenChecksGeometry.C,
//     polyMeshGenChecksTopology.C}
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (help::tetQuality)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae (2026-02-17).
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
// `tetrahedron::circumRadius` follows OpenFOAM's
//   src/OpenFOAM/meshes/primitiveShapes/tetrahedron/tetrahedronI.H
// (OpenFOAM, GPL-3.0-or-later; the source is not vendored in this workspace,
// so it is written from that routine and checked by the regular-tetrahedron
// test below: quality exactly 1).
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

//! cfMesh's **`polyMeshGenChecks`** (gh:#1261): every check, with its
//! threshold as an argument, and the `meshQualitySettings` that
//! `findBadFacesAdditionalChecks` reads.
//!
//! The checks the optimiser already used are in [`crate::mesh_optimizer`]
//! (`findBadFaces`, `findBadFacesRelaxed`, `findLowQualityFaces`,
//! `findWorstQualityFaces` with fixed thresholds); this module has their
//! threshold-taking forms, and the rest:
//!
//! | upstream | here |
//! |---|---|
//! | `checkClosedBoundary`, `checkClosedCells`, `checkCellVolumes`, `checkFaceAreas` | [`check_closed_boundary`], [`check_closed_cells`], [`check_cell_volumes`], [`check_face_areas`] |
//! | `checkFaceDotProduct`, `checkFaceSkewness`, `checkFacePyramids`, `checkFaceFlatness`, `checkCellPartTetrahedra` | [`check_face_dot_product`], [`check_face_skewness`], [`check_face_pyramids`], [`check_face_flatness`], [`check_cell_part_tetrahedra`] |
//! | `checkTetQuality`, `checkMinTwist`, `checkTriangleTwist`, `checkCellDeterminant`, `checkMinVolRatio`, `checkFaceUniformity`, `checkVolumeUniformity`, `checkFaceAngles` | [`check_tet_quality`], [`check_min_twist`], [`check_triangle_twist`], [`check_cell_determinant`], [`check_min_vol_ratio`], [`check_face_uniformity`], [`check_volume_uniformity`], [`check_face_angles`] |
//! | `findBadFacesAdditionalChecks` + `meshQualitySettings` | [`find_bad_faces_additional_checks`], [`MeshQualitySettings`] |
//! | `checkPoints`, `checkUpperTriangular`, `checkCellsZipUp`, `checkFaceVertices` | [`check_points`], [`check_upper_triangular`], [`check_cells_zip_up`], [`check_face_vertices`] |
//! | `checkGeometry`, `checkTopology`, `checkMesh` | [`check_geometry`], [`check_topology`], [`check_mesh`] |
//!
//! These are **cfMesh's definitions**; [`crate::checks`] holds OpenFOAM's
//! (`checkMesh` / `snappyHexMesh`'s `meshQualityControls`). Where both have
//! a check of the same name (twist, determinant, volume ratio) they differ,
//! and both are kept (the gh:#1261 differential in
//! `docs/upstream_coverage.md`).
//!
//! **Conventions.** Every face check returns one flag per face (upstream's
//! `labelHashSet`), every cell check one per cell, every point check one per
//! point. `active` is upstream's `changedFacePtr`/`activeFacePtr`: `None`
//! checks every face. Centres are this crate's (vertex-average face centre,
//! pyramid-decomposition cell centre, [`crate::checks::cell_geometry`]), where
//! cfMesh uses OpenFOAM's area-weighted face centre.
//!
//! **Upstream quirks kept, said so at each function:**
//! `checkCellDeterminant` checks no cell when no `changedFacePtr` is given;
//! `checkMinVolRatio`, `checkFaceUniformity` and `checkVolumeUniformity`
//! always return `false` (they only fill the set; `checkVolumeUniformity`
//! is an empty stub); `checkMinTwist`'s boundary loop ignores the active
//! flags; `checkFaceVertices` and `checkClosedCells` accept a label equal to
//! the list size (`> n`, not `>= n`).

use crate::checks::{cell_geometry, pmg_geometry, PmgGeometry};
use crate::correct_edges::foam_face_centre;
use crate::surface_engine::ring_area_vector;
use crate::math::Vec3;
use crate::mesh_optimizer::{face_dot_products_g, face_skewness_g};
use crate::volume_mesh::VolumeMesh;
use petir::mathf::RealMath;

/// OpenFOAM's `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;
/// OpenFOAM's `ROOTVSMALL` for `double`.
const ROOTVSMALL: f64 = 1e-150;
/// OpenFOAM's `GREAT` for `double`.
const GREAT: f64 = 1e15;

fn is_active(active: Option<&[bool]>, f: usize) -> bool {
    active.is_none_or(|a| a[f])
}

fn tet_volume(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    // `tetrahedron::mag`: `(1.0/6.0)*(((b - a) ^ (c - a)) & (d - a))`
    (1.0 / 6.0) * b.sub(a).cross(c.sub(a)).dot(d.sub(a))
}

/// OpenFOAM's `tetrahedron::circumRadius`; `GREAT` for a degenerate
/// tetrahedron.
pub fn tet_circum_radius(a0: Vec3, b0: Vec3, c0: Vec3, d0: Vec3) -> f64 {
    let a = b0.sub(a0);
    let b = c0.sub(a0);
    let c = d0.sub(a0);
    let lambda = c.dot(c) - a.dot(c);
    let mu = b.dot(b) - a.dot(b);
    let ba = b.cross(a);
    let ca = c.cross(a);
    let num = ba.scale(lambda).sub(ca.scale(mu));
    let denom = c.dot(ba);
    if denom.abs() < ROOTVSMALL {
        GREAT
    } else {
        a.scale(0.5).add(num.scale(1.0 / (2.0 * denom))).length()
    }
}

/// `help::tetQuality`: signed volume over that of the regular tetrahedron
/// with the same circumradius, `V / (8/(9√3) min(R, GREAT)³ + ROOTVSMALL)`;
/// 1 for a regular tetrahedron, negative for an inverted one.
pub fn tet_quality(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    let r = tet_circum_radius(a, b, c, d).min(GREAT);
    tet_volume(a, b, c, d) / (8.0 / (9.0 * 3.0f64.sqrt()) * r * r * r + ROOTVSMALL)
}

/// Each cell's faces, ascending.
fn cell_faces(mesh: &VolumeMesh) -> Vec<Vec<usize>> {
    let mut cf: Vec<Vec<usize>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cf[mesh.owner[f]].push(f);
        if let Some(n) = mesh.neighbour[f] {
            cf[n].push(f);
        }
    }
    cf
}

/// `checkClosedBoundary`: the boundary's area vectors sum to zero. True
/// (failed) when the largest component of the sum exceeds
/// `SMALL max(1, Σ|S|)` (upstream: "this is APPROXIMATE").
pub fn check_closed_boundary(mesh: &VolumeMesh) -> bool {
    let (mut sum, mut sum_mag) = (Vec3::ZERO, 0.0);
    for f in mesh.n_internal_faces()..mesh.face_count() {
        let s = mesh.face_area_vector(f);
        sum = sum.add(s);
        sum_mag += s.length();
    }
    let max_open = sum.x.max(sum.y.max(sum.z));
    max_open > SMALL * sum_mag.max(1.0)
}

/// `checkClosedCells(aspectWarn)` (upstream default 1000): the cells whose
/// area vectors do not sum to zero, and whether any cell's aspect ratio
/// `Σ|S| / (6 V^(2/3))` exceeds `aspect_warn`. Returns (failed, open cells).
/// A face label above the face count fails the check first (upstream's
/// `> nFaces`; this crate's cells are implicit, so it cannot happen).
pub fn check_closed_cells(mesh: &VolumeMesh, aspect_warn: f64) -> (bool, Vec<bool>) {
    let (vols, _) = cell_geometry(mesh);
    let mut sum = vec![Vec3::ZERO; mesh.n_cells];
    let mut sum_mag = vec![0.0; mesh.n_cells];
    for f in 0..mesh.face_count() {
        let s = mesh.face_area_vector(f);
        let o = mesh.owner[f];
        sum[o] = sum[o].add(s);
        sum_mag[o] += s.length();
        if let Some(n) = mesh.neighbour[f] {
            sum[n] = sum[n].sub(s);
            sum_mag[n] += s.length();
        }
    }
    let mut open = vec![false; mesh.n_cells];
    let (mut n_open, mut n_aspect) = (0usize, 0usize);
    for c in 0..mesh.n_cells {
        let max_open = sum[c].x.max(sum[c].y.max(sum[c].z));
        if max_open > SMALL * sum_mag[c].max(1.0) {
            open[c] = true;
            n_open += 1;
        }
        let aspect = sum_mag[c] / 6.0 / vols[c].r_powf(2.0 / 3.0);
        if aspect > aspect_warn {
            n_aspect += 1;
        }
    }
    (n_open > 0 || n_aspect > 0, open)
}

/// `checkCellVolumes`: the cells with volume below `VSMALL`; failed when
/// any.
pub fn check_cell_volumes(mesh: &VolumeMesh) -> (bool, Vec<bool>) {
    let (vols, _) = cell_geometry(mesh);
    let bad: Vec<bool> = vols.iter().map(|&v| v < VSMALL).collect();
    (bad.iter().any(|&b| b), bad)
}

/// `checkFaceAreas(minFaceArea)` (default `VSMALL`): faces with |S| below
/// it; failed when the smallest area is below `VSMALL`.
pub fn check_face_areas(mesh: &VolumeMesh, min_face_area: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_face_areas_g(mesh, &pmg_geometry(mesh), min_face_area, active)
}

/// [`check_face_areas`] on a precomputed [`PmgGeometry`]; `faceAreas` are
/// the addressing's (gh:#1385 round 3).
pub(crate) fn check_face_areas_g(mesh: &VolumeMesh, g: &PmgGeometry, min_face_area: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let mut bad = vec![false; mesh.face_count()];
    let mut min_area = f64::MAX;
    for (f, b) in bad.iter_mut().enumerate() {
        if !is_active(active, f) {
            continue;
        }
        let a = g.face_areas[f].length();
        *b = a < min_face_area;
        min_area = min_area.min(a);
    }
    (min_area < VSMALL, bad)
}

/// `checkFaceDotProduct(nonOrthWarn)` (default 70°): internal faces whose
/// `d·S/(|d||S|)` is below `cos(nonOrthWarn)` (the severe ones and, at or
/// below `SMALL`, the errors). Failed when any face is an error
/// (`errorNonOrth`); severe faces only go in the set.
pub fn check_face_dot_product(mesh: &VolumeMesh, non_orth_warn_deg: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_face_dot_product_g(mesh, &pmg_geometry(mesh), non_orth_warn_deg, active)
}

/// [`check_face_dot_product`] on a precomputed [`PmgGeometry`].
pub(crate) fn check_face_dot_product_g(mesh: &VolumeMesh, g: &PmgGeometry, non_orth_warn_deg: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let all = vec![true; mesh.face_count()];
    let dots = face_dot_products_g(mesh, g, active.unwrap_or(&all));
    // `::cos(nonOrthWarn/180.0*M_PI)`
    let threshold = (non_orth_warn_deg / 180.0 * std::f64::consts::PI).r_cos();
    let mut bad = vec![false; mesh.face_count()];
    let mut n_error = 0usize;
    for f in 0..mesh.n_internal_faces() {
        if !is_active(active, f) {
            continue;
        }
        if dots[f] < threshold {
            bad[f] = true;
            if dots[f] <= SMALL {
                n_error += 1;
            }
        }
    }
    (n_error > 0, bad)
}

/// `checkFaceSkewness(warnSkew)` (default 4): faces with cfMesh skewness
/// ([`face_skewness`]) above it.
pub fn check_face_skewness(mesh: &VolumeMesh, warn_skew: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_face_skewness_g(mesh, &pmg_geometry(mesh), warn_skew, active)
}

/// [`check_face_skewness`] on a precomputed [`PmgGeometry`].
pub(crate) fn check_face_skewness_g(mesh: &VolumeMesh, g: &PmgGeometry, warn_skew: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let all = vec![true; mesh.face_count()];
    let skew = face_skewness_g(mesh, g, active.unwrap_or(&all));
    let bad: Vec<bool> = (0..mesh.face_count()).map(|f| is_active(active, f) && skew[f] > warn_skew).collect();
    (bad.iter().any(|&b| b), bad)
}

/// `checkFacePyramids(minPyrVol)` (default `−SMALL`; the optimiser uses
/// `VSMALL`): the owner pyramid's volume `S·(fc − c_own)/3` must exceed
/// `minPyrVol`, and so must the neighbour's `S·(c_nei − fc)/3`.
pub fn check_face_pyramids(mesh: &VolumeMesh, min_pyr_vol: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_face_pyramids_g(mesh, &pmg_geometry(mesh), min_pyr_vol, active)
}

/// [`check_face_pyramids`] on a precomputed [`PmgGeometry`], as upstream:
/// `pyramidPointFaceRef(face, apex).mag(points)` is
/// `(1.0/3.0)*(face.areaNormal(points) & (apex - face.centre(points)))`
/// with OpenFOAM's `face::areaNormal` and `face::centre`; the owner's
/// pyramid (negative when valid) is bad above `-minPyrVol`, the neighbour's
/// below `minPyrVol`. Apexes are the addressing's cell centres.
pub(crate) fn check_face_pyramids_g(mesh: &VolumeMesh, g: &PmgGeometry, min_pyr_vol: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let mut bad = vec![false; mesh.face_count()];
    for (f, b) in bad.iter_mut().enumerate() {
        if !is_active(active, f) {
            continue;
        }
        let ring = &mesh.faces[f];
        let area = ring_area_vector(&mesh.points, ring);
        let fc = foam_face_centre(&mesh.points, ring);
        let pyr_own = (1.0 / 3.0) * area.dot(g.cell_centres[mesh.owner[f]].sub(fc));
        if pyr_own > -min_pyr_vol {
            *b = true;
        }
        if let Some(n) = mesh.neighbour[f] {
            let pyr_nei = (1.0 / 3.0) * area.dot(g.cell_centres[n].sub(fc));
            if pyr_nei < min_pyr_vol {
                *b = true;
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkFaceFlatness(warnFlatness)`: polygons (more than 3 points) whose
/// |S| over the summed areas of their centre triangles is below it.
pub fn check_face_flatness(mesh: &VolumeMesh, warn_flatness: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_face_flatness_g(mesh, &pmg_geometry(mesh), warn_flatness, active)
}

/// [`check_face_flatness`] on a precomputed [`PmgGeometry`] (upstream's
/// `faceCentres` and `mag(faceAreas)`; the triangle area is
/// `mag(0.5*((next - this)^(fc - this)))`).
pub(crate) fn check_face_flatness_g(mesh: &VolumeMesh, g: &PmgGeometry, warn_flatness: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let mut bad = vec![false; mesh.face_count()];
    for (f, b) in bad.iter_mut().enumerate() {
        let ring = &mesh.faces[f];
        let k = ring.len();
        if !is_active(active, f) || k <= 3 {
            continue;
        }
        let mag_area = g.face_areas[f].length();
        if mag_area <= VSMALL {
            continue;
        }
        let fc = g.face_centres[f];
        let mut sum_a = 0.0;
        for i in 0..k {
            let p = mesh.points[ring[i]];
            let q = mesh.points[ring[(i + 1) % k]];
            sum_a += q.sub(p).cross(fc.sub(p)).scale(0.5).length();
        }
        if mag_area / (sum_a + VSMALL) < warn_flatness {
            *b = true;
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkCellPartTetrahedra(minPartTet)` (default `VSMALL`): each face
/// triangle (edge, face centre) with each adjacent cell centre must make a
/// tetrahedron of volume above `minPartTet`.
pub fn check_cell_part_tetrahedra(mesh: &VolumeMesh, min_part_tet: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    check_cell_part_tetrahedra_g(mesh, &pmg_geometry(mesh), min_part_tet, active)
}

/// [`check_cell_part_tetrahedra`] on a precomputed [`PmgGeometry`].
pub(crate) fn check_cell_part_tetrahedra_g(mesh: &VolumeMesh, g: &PmgGeometry, min_part_tet: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let centres = &g.cell_centres;
    let mut bad = vec![false; mesh.face_count()];
    for (f, b) in bad.iter_mut().enumerate() {
        if !is_active(active, f) {
            continue;
        }
        let ring = &mesh.faces[f];
        let k = ring.len();
        let fc = g.face_centres[f];
        let co = centres[mesh.owner[f]];
        for i in 0..k {
            let p = mesh.points[ring[i]];
            let q = mesh.points[ring[(i + 1) % k]];
            if tet_volume(fc, q, p, co) < min_part_tet {
                *b = true;
            }
            if let Some(n) = mesh.neighbour[f] {
                if tet_volume(fc, p, q, centres[n]) < min_part_tet {
                    *b = true;
                }
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkTetQuality(minTetQuality)` (default `VSMALL`): every tetrahedron
/// of each face's fans from each of its points, with the owner centre (and
/// with the neighbour centre, the other way round), must have
/// [`tet_quality`] at least `minTetQuality`.
pub fn check_tet_quality(mesh: &VolumeMesh, min_tet_quality: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let (_, centres) = cell_geometry(mesh);
    let mut bad = vec![false; mesh.face_count()];
    for (f, b) in bad.iter_mut().enumerate() {
        if !is_active(active, f) {
            continue;
        }
        let ring = &mesh.faces[f];
        let k = ring.len();
        for pi in 0..k {
            for j in 1..k - 1 {
                let p = mesh.points[ring[pi]];
                let fj = mesh.points[ring[(pi + j) % k]];
                let nfj = mesh.points[ring[(pi + j + 1) % k]];
                if tet_quality(centres[mesh.owner[f]], p, fj, nfj) < min_tet_quality {
                    *b = true;
                    continue;
                }
                if let Some(n) = mesh.neighbour[f] {
                    if tet_quality(centres[n], p, nfj, fj) < min_tet_quality {
                        *b = true;
                    }
                }
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkMinTwist(minTwist)` (default `VSMALL`, must be in [−1, 1]): a
/// polygon fails when any of its centre triangles' unit normals has a dot
/// product below `minTwist` with the owner-to-neighbour direction (internal)
/// or the owner-centre-to-face-centre direction (boundary). On boundary
/// faces, as upstream: the active flags are ignored, a triangle face uses a
/// zero direction (so it fails whenever `minTwist > 0`), and the triangle's
/// own magnitude is not padded by `VSMALL`.
pub fn check_min_twist(mesh: &VolumeMesh, min_twist: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    assert!((-1.0 - SMALL..=1.0 + SMALL).contains(&min_twist), "minTwist should be [-1..1] but is now {min_twist}");
    let (_, centres) = cell_geometry(mesh);
    let mut bad = vec![false; mesh.face_count()];
    for f in 0..mesh.face_count() {
        let ring = &mesh.faces[f];
        let k = ring.len();
        let fc = mesh.face_centroid(f);
        let co = centres[mesh.owner[f]];
        match mesh.neighbour[f] {
            Some(n) => {
                if !is_active(active, f) || k <= 3 {
                    continue;
                }
                let d = centres[n].sub(co);
                let nf = d.scale(1.0 / (d.length() + VSMALL));
                for i in 0..k {
                    let p = mesh.points[ring[i]];
                    let q = mesh.points[ring[(i + 1) % k]];
                    let tri = q.sub(p).cross(fc.sub(p)).scale(0.5);
                    let m = tri.length() + VSMALL;
                    if m > VSMALL && nf.dot(tri.scale(1.0 / m)) < min_twist {
                        bad[f] = true;
                        break;
                    }
                }
            }
            None => {
                let nf = if k > 3 {
                    let d = fc.sub(co);
                    d.scale(1.0 / (d.length() + VSMALL))
                } else {
                    Vec3::ZERO
                };
                for i in 0..k {
                    let p = mesh.points[ring[i]];
                    let q = mesh.points[ring[(i + 1) % k]];
                    let tri = q.sub(p).cross(fc.sub(p)).scale(0.5);
                    let m = tri.length();
                    if m > VSMALL && nf.dot(tri.scale(1.0 / m)) < min_twist {
                        bad[f] = true;
                        break;
                    }
                }
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkTriangleTwist(minTwist)` (default `VSMALL`): a polygon fails when
/// two of its centre triangles' unit normals have a dot product below
/// `minTwist`.
pub fn check_triangle_twist(mesh: &VolumeMesh, min_twist: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    assert!((-1.0 - SMALL..=1.0 + SMALL).contains(&min_twist), "minTwist should be [-1..1] but is now {min_twist}");
    let mut bad = vec![false; mesh.face_count()];
    for (f, b) in bad.iter_mut().enumerate() {
        let ring = &mesh.faces[f];
        let k = ring.len();
        if !is_active(active, f) || k <= 3 {
            continue;
        }
        let fc = mesh.face_centroid(f);
        let normal = |i: usize| {
            let p = mesh.points[ring[i]];
            let q = mesh.points[ring[(i + 1) % k]];
            let n = q.sub(p).cross(fc.sub(p)).scale(0.5);
            n.scale(1.0 / (n.length() + VSMALL))
        };
        'outer: for i in 0..k {
            let ni = normal(i);
            for j in i + 1..k {
                if ni.dot(normal(j)) < min_twist {
                    *b = true;
                    break 'outer;
                }
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// The cfMesh cell determinant of every cell: `det(Σ S Ŝ / Σ|S|) / (1/27)`
/// over all its faces (1 for a cube).
pub fn cell_determinants(mesh: &VolumeMesh) -> Vec<f64> {
    cell_faces(mesh)
        .iter()
        .map(|faces| {
            let mut t = [[0.0f64; 3]; 3];
            let mut mag_sum = 0.0;
            for &f in faces {
                let s = mesh.face_area_vector(f);
                let m = s.length() + VSMALL;
                mag_sum += m;
                let a = [s.x, s.y, s.z];
                for i in 0..3 {
                    for j in 0..3 {
                        t[i][j] += a[i] * a[j] / m;
                    }
                }
            }
            let t = t.map(|r| r.map(|x| x / mag_sum));
            let det = t[0][0] * (t[1][1] * t[2][2] - t[1][2] * t[2][1]) - t[0][1] * (t[1][0] * t[2][2] - t[1][2] * t[2][0])
                + t[0][2] * (t[1][0] * t[2][1] - t[1][1] * t[2][0]);
            det / 0.037037037037037
        })
        .collect()
}

/// `checkCellDeterminant(warnDet)` (default 1e-3): every face of a cell
/// with [`cell_determinants`] below `warnDet`. **Upstream quirk kept:** only
/// cells next to an active face are checked, and with `active = None` no
/// cell is (upstream marks `affectedCells` only from `changedFacePtr`).
pub fn check_cell_determinant(mesh: &VolumeMesh, warn_det: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let mut affected = vec![false; mesh.n_cells];
    if let Some(a) = active {
        for f in 0..mesh.face_count() {
            if a[f] {
                affected[mesh.owner[f]] = true;
                if let Some(n) = mesh.neighbour[f] {
                    affected[n] = true;
                }
            }
        }
    }
    let dets = cell_determinants(mesh);
    let cf = cell_faces(mesh);
    let mut bad = vec![false; mesh.face_count()];
    let mut n_warn = 0usize;
    for c in 0..mesh.n_cells {
        if affected[c] && dets[c] < warn_det {
            n_warn += 1;
            for &f in &cf[c] {
                bad[f] = true;
            }
        }
    }
    (n_warn > 0, bad)
}

/// `checkMinVolRatio(mesh, volRatio, changedFacePtr)`: `min(V)/max(V)` of
/// the two cells of each active internal face; 1 elsewhere on internal
/// faces (boundary entries are left 0 here; upstream leaves them unset).
pub fn vol_ratios(mesh: &VolumeMesh, active: Option<&[bool]>) -> Vec<f64> {
    let (vols, _) = cell_geometry(mesh);
    (0..mesh.face_count())
        .map(|f| match mesh.neighbour[f] {
            Some(n) if is_active(active, f) => {
                let (o, ne) = (vols[mesh.owner[f]].abs(), vols[n].abs());
                o.min(ne) / o.max(ne)
            }
            Some(_) => 1.0,
            None => 0.0,
        })
        .collect()
}

/// `checkMinVolRatio(warnVolRatio)` (default 0.01): internal faces whose
/// [`vol_ratios`] is below it. **Upstream returns `false` always**; so does
/// this (the flags are the result).
pub fn check_min_vol_ratio(mesh: &VolumeMesh, warn_vol_ratio: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let r = vol_ratios(mesh, active);
    let bad = (0..mesh.face_count()).map(|f| mesh.neighbour[f].is_some() && is_active(active, f) && r[f] < warn_vol_ratio).collect();
    (false, bad)
}

/// `checkFaceUniformity(mesh, faceUniformity, changedFacePtr)`:
/// `min(d_own, d_nei)/(d_own + d_nei)` (centre-to-face-centre distances) on
/// active internal faces, 0.5 elsewhere.
pub fn face_uniformity(mesh: &VolumeMesh, active: Option<&[bool]>) -> Vec<f64> {
    let (_, centres) = cell_geometry(mesh);
    (0..mesh.face_count())
        .map(|f| match mesh.neighbour[f] {
            Some(n) if is_active(active, f) => {
                let fc = mesh.face_centroid(f);
                let d_own = centres[mesh.owner[f]].sub(fc).length();
                let d_nei = centres[n].sub(fc).length();
                d_own.min(d_nei) / (d_own + d_nei)
            }
            _ => 0.5,
        })
        .collect()
}

/// `checkFaceUniformity(warnUniform)` (default 0.1): internal faces whose
/// [`face_uniformity`] is below it. Upstream returns `false` always.
pub fn check_face_uniformity(mesh: &VolumeMesh, warn_uniform: f64, active: Option<&[bool]>) -> (bool, Vec<bool>) {
    let u = face_uniformity(mesh, active);
    let bad = (0..mesh.face_count()).map(|f| mesh.neighbour[f].is_some() && is_active(active, f) && u[f] < warn_uniform).collect();
    (false, bad)
}

/// `checkVolumeUniformity`: an empty stub upstream (returns `false`, sets
/// nothing); kept as one so `meshQualitySettings`' `volUniform` has the
/// upstream effect, none.
pub fn check_volume_uniformity(mesh: &VolumeMesh, _warn_uniform: f64, _active: Option<&[bool]>) -> (bool, Vec<bool>) {
    (false, vec![false; mesh.face_count()])
}

/// `checkFaceAngles(maxDeg)` (default 10°, must be in [0, 180]): a face is
/// concave at a point when the cross product of its two edges there is
/// longer than `sin(maxDeg)` and points against the face normal. Returns
/// (any concave face, flags, the largest such sine).
pub fn check_face_angles(mesh: &VolumeMesh, max_deg: f64, active: Option<&[bool]>) -> (bool, Vec<bool>, f64) {
    assert!((-SMALL..=180.0 + SMALL).contains(&max_deg), "maxDeg should be [0..180] but is now {max_deg}");
    let max_sin = (max_deg / 180.0 * std::f64::consts::PI).r_sin();
    let mut bad = vec![false; mesh.face_count()];
    let mut max_edge_sin = 0.0f64;
    for (f, b) in bad.iter_mut().enumerate() {
        if !is_active(active, f) {
            continue;
        }
        let ring = &mesh.faces[f];
        let k = ring.len();
        let s = mesh.face_area_vector(f);
        let n = s.scale(1.0 / (s.length() + VSMALL));
        let mut e_prev = mesh.points[ring[0]].sub(mesh.points[ring[k - 1]]);
        let mut mag_prev = e_prev.length();
        e_prev = e_prev.scale(1.0 / (mag_prev + VSMALL));
        for fp0 in 0..k {
            let fp1 = (fp0 + 1) % k;
            let mut e10 = mesh.points[ring[fp1]].sub(mesh.points[ring[fp0]]);
            let mag10 = e10.length();
            e10 = e10.scale(1.0 / (mag10 + VSMALL));
            if mag_prev > SMALL && mag10 > SMALL {
                let en = e_prev.cross(e10);
                let m = en.length();
                if m >= max_sin && en.scale(1.0 / m).dot(n) < SMALL {
                    *b = true;
                    max_edge_sin = max_edge_sin.max(m);
                }
            }
            e_prev = e10;
            mag_prev = mag10;
        }
    }
    (bad.iter().any(|&b| b), bad, max_edge_sin)
}

/// `meshDict`'s `meshQualitySettings`: each check
/// `findBadFacesAdditionalChecks` runs when its entry is given (`None`: not
/// run, as when the dictionary lacks it). The upstream defaults it would use
/// are in the field docs, but the checks only run when set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeshQualitySettings {
    /// `maxNonOrthogonality` (65) → [`check_face_dot_product`].
    pub max_non_orthogonality: Option<f64>,
    /// `maxSkewness` (2) → [`check_face_skewness`].
    pub max_skewness: Option<f64>,
    /// `fcUniform` (0.1) → [`check_face_uniformity`].
    pub fc_uniform: Option<f64>,
    /// `volUniform` (0.1) → [`check_volume_uniformity`] (a stub).
    pub vol_uniform: Option<f64>,
    /// `maxAngle` (10) → [`check_face_angles`].
    pub max_angle: Option<f64>,
    /// `minTetQuality` (`VSMALL`) → [`check_tet_quality`].
    pub min_tet_quality: Option<f64>,
    /// `minFaceTwist` (`VSMALL`) → [`check_min_twist`].
    pub min_face_twist: Option<f64>,
    /// `minCellDeterminant` (1e-3) → [`check_cell_determinant`].
    pub min_cell_determinant: Option<f64>,
    /// `minVolRatio` (0.01) → [`check_min_vol_ratio`].
    pub min_vol_ratio: Option<f64>,
    /// `minFaceTriangleTwist` (`VSMALL`) → [`check_triangle_twist`].
    pub min_face_triangle_twist: Option<f64>,
    /// `minPyramidVolume` (`VSMALL`) → [`check_face_pyramids`].
    pub min_pyramid_volume: Option<f64>,
    /// `minimumFaceArea` (`VSMALL`) → [`check_face_areas`].
    pub minimum_face_area: Option<f64>,
    /// `faceFlatness` (0.8) → [`check_face_flatness`].
    pub face_flatness: Option<f64>,
    /// `minCellPartTetrahedra` (`VSMALL`) → [`check_cell_part_tetrahedra`].
    pub min_cell_part_tetrahedra: Option<f64>,
}

/// `findBadFacesAdditionalChecks`: the union of the checks `settings` names,
/// in upstream's order. All `None` (the default) flags nothing, as upstream
/// without `meshQualitySettings`.
pub fn find_bad_faces_additional_checks(mesh: &VolumeMesh, settings: &MeshQualitySettings, active: Option<&[bool]>) -> Vec<bool> {
    let mut bad = vec![false; mesh.face_count()];
    let mut or = |flags: Vec<bool>| {
        for (b, f) in bad.iter_mut().zip(flags) {
            *b |= f;
        }
    };
    let s = settings;
    if let Some(v) = s.max_non_orthogonality {
        or(check_face_dot_product(mesh, v, active).1);
    }
    if let Some(v) = s.max_skewness {
        or(check_face_skewness(mesh, v, active).1);
    }
    if let Some(v) = s.fc_uniform {
        or(check_face_uniformity(mesh, v, active).1);
    }
    if let Some(v) = s.vol_uniform {
        or(check_volume_uniformity(mesh, v, active).1);
    }
    if let Some(v) = s.max_angle {
        or(check_face_angles(mesh, v, active).1);
    }
    if let Some(v) = s.min_tet_quality {
        or(check_tet_quality(mesh, v, active).1);
    }
    if let Some(v) = s.min_face_twist {
        or(check_min_twist(mesh, v, active).1);
    }
    if let Some(v) = s.min_cell_determinant {
        or(check_cell_determinant(mesh, v, active).1);
    }
    if let Some(v) = s.min_vol_ratio {
        or(check_min_vol_ratio(mesh, v, active).1);
    }
    if let Some(v) = s.min_face_triangle_twist {
        or(check_triangle_twist(mesh, v, active).1);
    }
    if let Some(v) = s.min_pyramid_volume {
        or(check_face_pyramids(mesh, v, active).1);
    }
    if let Some(v) = s.minimum_face_area {
        or(check_face_areas(mesh, v, active).1);
    }
    if let Some(v) = s.face_flatness {
        or(check_face_flatness(mesh, v, active).1);
    }
    if let Some(v) = s.min_cell_part_tetrahedra {
        or(check_cell_part_tetrahedra(mesh, v, active).1);
    }
    bad
}

/// `findBadFaces` with `meshQualitySettings`: the additional checks, then
/// pyramids (`VSMALL`), flatness 0.8, cell-part tetrahedra (`VSMALL`), areas
/// (`VSMALL`). With default settings it is
/// [`crate::mesh_optimizer::find_bad_faces`] (pinned by a test).
pub fn find_bad_faces_with(mesh: &VolumeMesh, settings: &MeshQualitySettings, active: Option<&[bool]>) -> Vec<bool> {
    let mut bad = find_bad_faces_additional_checks(mesh, settings, active);
    for flags in [
        check_face_pyramids(mesh, VSMALL, active).1,
        check_face_flatness(mesh, 0.8, active).1,
        check_cell_part_tetrahedra(mesh, VSMALL, active).1,
        check_face_areas(mesh, VSMALL, active).1,
    ] {
        for (b, f) in bad.iter_mut().zip(flags) {
            *b |= f;
        }
    }
    bad
}

/// `checkPoints`: points used by no face or by no cell; failed when any.
pub fn check_points(mesh: &VolumeMesh) -> (bool, Vec<bool>) {
    // A point is in a cell exactly when it is in a face (every face has an
    // owner), so the two upstream loops flag the same points here.
    let mut used = vec![false; mesh.points.len()];
    for ring in &mesh.faces {
        for &p in ring {
            used[p] = true;
        }
    }
    let bad: Vec<bool> = used.iter().map(|&u| !u).collect();
    (bad.iter().any(|&b| b), bad)
}

/// `checkUpperTriangular`: every internal face has `owner < neighbour`, and
/// the internal faces are in upper-triangular order (by owner, then by
/// neighbour). Flags the faces with `owner >= neighbour` and the first face
/// out of order.
pub fn check_upper_triangular(mesh: &VolumeMesh) -> (bool, Vec<bool>) {
    let n_int = mesh.n_internal_faces();
    let mut bad = vec![false; mesh.face_count()];
    let mut error = false;
    for f in 0..n_int {
        let n = mesh.neighbour[f].expect("internal face");
        if mesh.owner[f] >= n {
            bad[f] = true;
            error = true;
        }
    }
    // cellCells per cell, from its internal faces in face order.
    let cf = cell_faces(mesh);
    let other = |c: usize, f: usize| if mesh.owner[f] == c { mesh.neighbour[f] } else { Some(mesh.owner[f]) };
    let mut check: Vec<Option<usize>> = vec![None; n_int];
    let mut n_checks = 0usize;
    for c in 0..mesh.n_cells {
        let cc: Vec<usize> = cf[c].iter().filter(|&&f| f < n_int).filter_map(|&f| other(c, f)).collect();
        let mut used = vec![false; cc.len()];
        for _ in 0..cc.len() {
            let (mut next, mut min_nei) = (None, mesh.n_cells);
            for (i, &nb) in cc.iter().enumerate() {
                if nb > c && !used[i] && nb < min_nei {
                    next = Some(i);
                    min_nei = nb;
                }
            }
            if let Some(i) = next {
                used[i] = true;
                if let Some(&f) = cf[c].iter().find(|&&f| f < n_int && mesh.neighbour[f] == Some(cc[i])) {
                    check[n_checks] = Some(f);
                    n_checks += 1;
                }
            }
        }
    }
    if let Some(f) = (0..n_int).find(|&f| check[f] != Some(f)) {
        bad[f] = true;
        error = true;
    }
    (error, bad)
}

/// `checkCellsZipUp`: cells in which some edge is used by only one of its
/// faces (open), or by more than two (flagged, but not counted as open, as
/// upstream). Failed when any cell is open.
pub fn check_cells_zip_up(mesh: &VolumeMesh) -> (bool, Vec<bool>) {
    let cf = cell_faces(mesh);
    let mut bad = vec![false; mesh.n_cells];
    let mut n_open = 0usize;
    for c in 0..mesh.n_cells {
        let mut edges: Vec<((usize, usize), usize)> = Vec::new();
        for &f in &cf[c] {
            let ring = &mesh.faces[f];
            let k = ring.len();
            for i in 0..k {
                let (a, b) = (ring[i], ring[(i + 1) % k]);
                match edges.iter_mut().find(|((x, y), _)| (*x == a && *y == b) || (*x == b && *y == a)) {
                    Some(e) => e.1 += 1,
                    None => edges.push(((a, b), 1)),
                }
            }
        }
        let mut single = false;
        for (_, n) in &edges {
            if *n == 1 {
                single = true;
            } else if *n != 2 {
                bad[c] = true;
            }
        }
        if single {
            bad[c] = true;
            n_open += 1;
        }
    }
    (n_open > 0, bad)
}

/// `checkFaceVertices`: faces with a label outside `0..=nPoints` (upstream
/// tests `> nPoints`, so a label equal to the point count passes: kept) or
/// a repeated label.
pub fn check_face_vertices(mesh: &VolumeMesh) -> (bool, Vec<bool>) {
    let np = mesh.points.len();
    let mut bad = vec![false; mesh.face_count()];
    for (f, ring) in mesh.faces.iter().enumerate() {
        if ring.iter().any(|&p| p > np) {
            bad[f] = true;
        }
        let mut seen = std::collections::BTreeSet::new();
        for &p in ring {
            if !seen.insert(p) {
                bad[f] = true;
            }
        }
    }
    (bad.iter().any(|&b| b), bad)
}

/// `checkGeometry`: the number of failed checks among closed boundary,
/// closed cells (aspect 1000), face areas, cell volumes, dot product (70°),
/// face uniformity, pyramids (`−SMALL`), skewness (4), cell-part
/// tetrahedra (`VSMALL`); upstream returns whether any failed.
pub fn check_geometry(mesh: &VolumeMesh) -> usize {
    [
        check_closed_boundary(mesh),
        check_closed_cells(mesh, 1000.0).0,
        check_face_areas(mesh, VSMALL, None).0,
        check_cell_volumes(mesh).0,
        check_face_dot_product(mesh, 70.0, None).0,
        check_face_uniformity(mesh, 0.1, None).0,
        check_face_pyramids(mesh, -SMALL, None).0,
        check_face_skewness(mesh, 4.0, None).0,
        check_cell_part_tetrahedra(mesh, VSMALL, None).0,
    ]
    .iter()
    .filter(|&&b| b)
    .count()
}

/// `checkTopology`: the number of failed checks among points, upper
/// triangular order, zipped-up cells and face vertices.
pub fn check_topology(mesh: &VolumeMesh) -> usize {
    [check_points(mesh).0, check_upper_triangular(mesh).0, check_cells_zip_up(mesh).0, check_face_vertices(mesh).0].iter().filter(|&&b| b).count()
}

/// `checkMesh`: topology then geometry; true when any check failed.
pub fn check_mesh(mesh: &VolumeMesh) -> bool {
    let t = check_topology(mesh);
    let g = check_geometry(mesh);
    t + g > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartesian::cartesian_box;
    use crate::mesh_optimizer::find_bad_faces;

    /// V&V (gh:#1261) — the plan's analytic cases. Predictions (written
    /// 2026-11-05, before the first run):
    /// - a regular tetrahedron has `tet_quality` 1 to 1e-12 (this also
    ///   checks the circumradius written from OpenFOAM's routine);
    /// - on a unit cube (one cell): cell determinant 1 to 1e-12, every face
    ///   passes `checkMinTwist(1 − 1e-12)` (all triangle normals parallel to
    ///   the centre direction), and on the 2×1×1 box the internal face's
    ///   volume ratio is exactly 1;
    /// - one internal quad of a 2×1×1 box folded about its diagonal by θ =
    ///   30° (one corner moved out of plane): `checkTriangleTwist` flags it
    ///   at a threshold just above the dot product of its two centre
    ///   triangles' normals and not just below; that dot product is computed
    ///   independently from the four corners;
    /// - `checkUpperTriangular` passes on `cartesian_box` and fails when two
    ///   internal faces are swapped.
    #[test]
    fn analytic_checks() {
        let s3 = 3.0f64.sqrt();
        let (a, b, c, d) = (Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, -1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0), Vec3::new(-1.0, -1.0, 1.0));
        let q = tet_quality(a, b, c, d).abs();
        println!("regular tet: quality {q}, circumradius {} (expect {s3})", tet_circum_radius(a, b, c, d));
        assert!((q - 1.0).abs() < 1e-12, "{q}");
        assert!((tet_circum_radius(a, b, c, d) - s3).abs() < 1e-12);

        let cube = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [1, 1, 1]);
        let det = cell_determinants(&cube)[0];
        println!("unit cube determinant {det}");
        assert!((det - 1.0).abs() < 1e-12);
        assert!(!check_min_twist(&cube, 1.0 - 1e-12, None).0);
        assert!(!check_mesh(&cube), "a unit cube passes checkMesh");

        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(2.0, 1.0, 1.0), [2, 1, 1]);
        assert_eq!(vol_ratios(&m, None)[0], 1.0);
        assert!(!check_upper_triangular(&m).0);
        // Fold the internal face x = 1: move its corner (1, 1, 1) along x.
        let p = m.points.iter().position(|q| q.sub(Vec3::new(1.0, 1.0, 1.0)).length() < 1e-12).unwrap();
        let theta = 30f64.to_radians();
        m.points[p] = Vec3::new(1.0 + theta.tan() * 2f64.sqrt(), 1.0, 1.0);
        let f = 0; // the only internal face
        let ring = &m.faces[f];
        let fc = m.face_centroid(f);
        let k = ring.len();
        let mut min_dot = 1.0f64;
        for i in 0..k {
            for j in i + 1..k {
                let n = |i: usize| {
                    let (p0, p1) = (m.points[ring[i]], m.points[ring[(i + 1) % k]]);
                    let v = p1.sub(p0).cross(fc.sub(p0));
                    v.scale(1.0 / v.length())
                };
                min_dot = min_dot.min(n(i).dot(n(j)));
            }
        }
        println!("folded face: smallest triangle-normal dot product {min_dot}");
        assert!(check_triangle_twist(&m, min_dot + 1e-9, None).1[f]);
        assert!(!check_triangle_twist(&m, min_dot - 1e-9, None).1[f]);
    }

    /// `checkUpperTriangular` against a mesh with two internal faces
    /// swapped (2×2×1 box: 4 internal faces). ~~Prediction: passes before the
    /// swap, fails after it.~~ **MISSED (first run, 2026-11-05):**
    /// `cartesian_box` fails it before any swap. It numbers its internal
    /// faces by direction (every x face, then y, then z), not in OpenFOAM's
    /// upper-triangular order (by owner, then neighbour); every face has
    /// owner < neighbour, so only the ordering half fails. The check is now
    /// made on the box with its internal faces sorted into that order
    /// (passes), then with two of them swapped (fails), and the unsorted box
    /// is recorded failing on order only. **UPDATED 2026-11-05 (gh:#1285):**
    /// ~~`cartesian_box` fails it~~ `cartesian_box` now ends in that order
    /// (`renumber_mesh::upper_triangular`) and passes as built (asserted);
    /// the sort below is then the identity.
    #[test]
    fn upper_triangular_detects_a_swap() {
        let raw = cartesian_box(Vec3::ZERO, Vec3::new(2.0, 2.0, 1.0), [2, 2, 1]);
        let (fail_raw, bad_raw) = check_upper_triangular(&raw);
        assert!(!fail_raw, "gh:#1285: cartesian_box is upper-triangular as built");
        let n_int = raw.n_internal_faces();
        assert!((0..n_int).all(|f| raw.owner[f] < raw.neighbour[f].unwrap()));
        println!("cartesian_box as built: upper-triangular {}, flagged {:?}", !fail_raw, (0..bad_raw.len()).filter(|&f| bad_raw[f]).collect::<Vec<_>>());
        let mut order: Vec<usize> = (0..n_int).collect();
        order.sort_by_key(|&f| (raw.owner[f], raw.neighbour[f]));
        let mut m = raw.clone();
        for (i, &f) in order.iter().enumerate() {
            m.faces[i] = raw.faces[f].clone();
            m.owner[i] = raw.owner[f];
            m.neighbour[i] = raw.neighbour[f];
        }
        assert!(!check_upper_triangular(&m).0);
        assert!(!check_cells_zip_up(&m).0 && !check_points(&m).0 && !check_face_vertices(&m).0);
        assert!(n_int >= 2);
        let mut s = m.clone();
        s.faces.swap(0, n_int - 1);
        s.owner.swap(0, n_int - 1);
        s.neighbour.swap(0, n_int - 1);
        let (fail, bad) = check_upper_triangular(&s);
        println!("swapped: fail {fail}, flagged {:?}", (0..bad.len()).filter(|&f| bad[f]).collect::<Vec<_>>());
        assert!(fail);
    }

    /// Pin (gh:#1261): with no `meshQualitySettings`, [`find_bad_faces_with`]
    /// is [`crate::mesh_optimizer::find_bad_faces`] bit for bit, on a clean
    /// box and on a box whose centre point was moved through its neighbour
    /// layer. Prediction: identical flags on both (the same formulas).
    #[test]
    fn find_bad_faces_with_default_settings_is_find_bad_faces() {
        let mut m = cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [4, 4, 4]);
        let s = MeshQualitySettings::default();
        assert_eq!(find_bad_faces_with(&m, &s, None), find_bad_faces(&m));
        let c = m.points.iter().position(|p| p.sub(Vec3::new(0.5, 0.5, 0.5)).length() < 1e-12).unwrap();
        m.points[c] = Vec3::new(0.5, 0.5, 0.9);
        let a = find_bad_faces_with(&m, &s, None);
        assert!(a.iter().any(|&b| b));
        assert_eq!(a, find_bad_faces(&m));
        // A setting adds faces, never removes them.
        let s2 = MeshQualitySettings { max_non_orthogonality: Some(20.0), min_cell_determinant: Some(0.5), ..Default::default() };
        let b = find_bad_faces_with(&m, &s2, None);
        assert!(a.iter().zip(&b).all(|(x, y)| !x || *y));
        // checkCellDeterminant's upstream quirk: no active flags, no cell.
        assert!(!check_cell_determinant(&m, 2.0, None).0);
        assert!(check_cell_determinant(&m, 2.0, Some(&vec![true; m.face_count()])).0);
    }
}
