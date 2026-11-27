// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// Ported from OpenFOAM ESI (gitlab.com/openfoam/core/openfoam, commit
// 6e2595746a396d688edd9890f4aebaaa7ff4ecfd), GPL-3.0-or-later:
//   src/meshTools/AMIInterpolation/faceAreaIntersect/faceAreaIntersect.{C,H}
//   src/meshTools/AMIInterpolation/faceAreaIntersect/faceAreaIntersectI.H
//   src/meshTools/AMIInterpolation/AMIInterpolation/advancingFrontAMI/
//       advancingFrontAMI.{C,H}, advancingFrontAMII.H   (serial parts)
//   src/meshTools/AMIInterpolation/AMIInterpolation/faceAreaWeightAMI/
//       faceAreaWeightAMI.{C,H}
//   src/meshTools/AMIInterpolation/AMIInterpolation/AMIInterpolation.{C,H},
//       AMIInterpolationI.H   (normaliseWeights, mustMatchFaces, calculate)
//   src/meshTools/AMIInterpolation/AMIInterpolation/findNearestMaskedOp.H
// and the OpenFOAM primitives those call:
//   src/OpenFOAM/meshes/meshShapes/face/{face.C, faceI.H, face.txx,
//       faceIntersection.C}   (areaNormal, centre, average, split/triangles,
//       nearestPointClassify)
//   src/OpenFOAM/meshes/primitiveShapes/triangle/triangleI.H
//       (areaNormal, centre, mag, nearestPointClassify)
//   src/OpenFOAM/meshes/primitiveShapes/plane/{plane.C, planeI.H}
//   src/OpenFOAM/meshes/boundBox/{boundBoxI.H, boundBox.txx}
//   src/OpenFOAM/meshes/primitiveMesh/PrimitivePatch/
//       PrimitivePatchAddressing.C, PrimitivePatchPointAddressing.C,
//       PrimitivePatchMeshData.C   (faceFaces, pointFaces, faceCentres,
//       faceNormals)
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

//! General 3-D AMI weights: a line-for-line port of OpenFOAM ESI's
//! `faceAreaWeightAMI` (advancing-front walk + `faceAreaIntersect`
//! triangle clipping), serial path only (gh:#1859).
//!
//! [`face_area_weight_ami`] takes two patches as plain geometry
//! ([`AmiPatchGeometry`]: points `[m]` and face vertex lists) and returns
//! upstream's `srcAddress`, `srcWeights`, `srcWeightsSum`, `srcMagSf`,
//! `tgtAddress`, `tgtWeights`, `tgtWeightsSum`, `tgtMagSf` (and the
//! per-overlap centroids and the raw, un-normalised overlap areas), normalised
//! exactly as `AMIInterpolation::normaliseWeights(requireMatch, true)`
//! normalises them.
//!
//! ## How upstream computes the weights (and this port with it)
//!
//! 1. **Triangulate** every face (`advancingFrontAMI::triangulatePatch`):
//!    `tmMesh` (default) splits at the most concave vertex, recursively
//!    (`face::split`); `tmFan` fans from vertex 0. The face area `magSf` is the
//!    sum of the triangle area vectors **projected on the face unit normal**
//!    (`areaNormalisationMode::project`, default) or their magnitudes (`mag`).
//! 2. **Intersect** a source face with a target face
//!    (`faceAreaIntersect::calc`): every source triangle is clipped by the
//!    three planes that contain a target-triangle edge and the pair's "crude
//!    resultant normal" `n = normalise(-n_src ± n_tgt)`. The clipped area is
//!    measured **in the source triangle's plane**; a vertex within
//!    `1e-6·sqrt(area_src_tri)` of a cutting plane counts as on it.
//! 3. **Walk** (`faceAreaWeightAMI::calcAddressing`): starting from source
//!    face 0 and target face 0, each source face is intersected with a target
//!    seed and its edge-neighbours (LIFO), growing through every target face
//!    whose overlap fraction exceeds `1e-6`; the next source face is chosen from
//!    the current one's neighbours, then from stored seeds, then by a
//!    nearest-face search.
//! 4. **Restart** (`restartUncoveredSourceFace`, default on): every source face
//!    whose covered fraction is below `0.95` is re-walked from the target face
//!    nearest each of its vertices.
//! 5. **Normalise** (`normaliseWeights(conformal = requireMatch)`):
//!    `weightsSum[f] = Σ area / magSf[f]` and the weights are divided by
//!    `magSf[f]`, or (when `requireMatch`) by `Σ area` so they sum to exactly 1.
//!
//! ## What is not ported
//!
//! - Everything parallel: `distributed()` patches, `createExtendedTgtPatch`,
//!   `advancingFrontAMIParallelOps.C`, `mapDistribute`, `nonConformalCorrection`
//!   (a no-op in serial).
//! - `cacheIntersections_` / debug OBJ output, `projectPointsToSurface`
//!   (`surfPtr`), the AMI cache, agglomeration.
//! - `lowWeightCorrection` is **applied at interpolation time** upstream
//!   (`AMIInterpolationTemplates.C` / `cyclicAMIPolyPatch`), not in the
//!   weights. Here it only does what it does inside the weight computation:
//!   it switches off `mustMatchFaces()` (so a stalled walk is not an error)
//!   and counts the faces whose weight sum is below it
//!   ([`FaceAreaWeightAmiResult::src_n_low_weight`]).
//! - The `indexedOctree` seed search is replaced by a **brute-force** nearest
//!   search over all target faces, with upstream's own distance
//!   (`face::nearestPoint`), search radius (`0.25·|bb|²`, then `GREAT²`),
//!   exclusion list and strict `<` update. It returns the same face as the
//!   octree whenever the nearest face is unique; on an exact distance tie the
//!   brute force keeps the **lowest index**, while the octree keeps the first
//!   face it visits in tree order. Ties happen in practice when the restart
//!   searches from a source vertex lying exactly on a target edge or vertex;
//!   the walk from either seed reaches the same overlapping set, so only the
//!   **order** of entries in an address list can differ from upstream there.
//!
//! ## Verification
//!
//! - **Code-to-code:** bit-identical to the unmodified ESI `faceAreaWeightAMI`
//!   (commit above, serial, g++ 13 `-O3`) on **8579 of 8579** output values in
//!   17 cases (planar, unstructured with a hexagon, concave L, cylindrical,
//!   identity, stalled walk with and without restart, `minCosAngle`,
//!   `maxDistance2` + `lowWeightCorrection`), every address list equal in
//!   order (2026-11-27). Driver, reference and per-case table:
//!   `verification_and_validation/upstream_ami_gh1859/`; test
//!   `tests/ami_face_area_weight_vs_openfoam.rs`.
//! - **Properties:** the `vv_*` unit tests below (identity, agreement with
//!   [`overlap_weights_1d`](crate::mesh::ami::overlap_weights_1d), unit weight
//!   sums on covered seams, a cylindrical seam against a semi-analytic
//!   facet-projection model).
//!
//! **Untrusted AI-assisted draft pending human V&V review** (2026-11-27).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use crate::mesh::ami::{AmiCoupling, AmiWeight};
use crate::primitives::Vector3;
use core::f64::consts::PI;
use petir::mathf::RealMath;

/// OpenFOAM `doubleScalarROOTVSMALL`.
const ROOTVSMALL: f64 = 1.0e-150;
/// OpenFOAM `doubleScalarVSMALL`.
const VSMALL: f64 = 1.0e-300;
/// OpenFOAM `doubleScalarGREAT`.
const GREAT: f64 = 1.0e15;

/// `faceAreaIntersect::tol`: fraction of the local length scale used as the
/// coplanarity tolerance when clipping, and the minimum overlap fraction of a
/// source face for a pair to be stored (dimensionless).
pub const FACE_AREA_INTERSECT_TOLERANCE: f64 = 1e-6;

/// `faceAreaWeightAMI::restartUncoveredSourceFace`'s `minWeight`: source faces
/// covered less than this are re-walked.
const RESTART_MIN_WEIGHT: f64 = 0.95;

/// `advancingFrontAMI::checkPatches`'s `maxBoundsError`.
const MAX_BOUNDS_ERROR: f64 = 0.05;

type Tri = [Vector3; 3];

/// How a face is split into triangles before clipping.
/// Mirrors `faceAreaIntersect::triangulationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmiTriangulationMode {
    /// `tmFan`: fan from the face's first vertex.
    Fan,
    /// `tmMesh` (upstream default): `face::triangles`, splitting recursively
    /// at the most concave vertex.
    #[default]
    Mesh,
}

/// How the face area `magSf` (the normaliser of the weights) is computed.
/// Mirrors `advancingFrontAMI::areaNormalisationMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmiAreaNormalisation {
    /// `project` (upstream default): Σ triangle area vectors · face unit normal.
    #[default]
    Project,
    /// `mag`: Σ |triangle area vectors|.
    Mag,
}

/// Which half of an AMI pair a face or patch belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmiPatchSide {
    /// The source patch (upstream `srcPatch`).
    Source,
    /// The target patch (upstream `tgtPatch`).
    Target,
}

/// One AMI patch as plain geometry: the `primitivePatch` upstream walks.
///
/// Faces index into `points`; a face's vertices are listed in order around it
/// and its normal follows the right-hand rule (OpenFOAM's convention: the
/// outward normal of the owner cell). Points not referenced by a face are
/// ignored, as upstream ignores points outside `meshPoints()`.
#[derive(Debug, Clone, PartialEq)]
pub struct AmiPatchGeometry {
    /// Point coordinates `[m]`.
    pub points: Vec<Vector3>,
    /// Face vertex lists (indices into [`points`](Self::points)), at least 3 each.
    pub faces: Vec<Vec<usize>>,
}

/// Options of the `faceAreaWeightAMI` constructor and of `advancingFrontAMI`'s
/// dictionary. [`Default`] gives upstream's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceAreaWeightAmiOptions {
    /// `requireMatch` (default `true`): every source face is expected to find
    /// a target face. Also selects **conformal** normalisation: each face's
    /// weights are divided by their own sum, so they sum to exactly 1.
    pub require_match: bool,
    /// `reverseTarget` (default `false`): `false` expects the two patches to
    /// face each other (opposite normals, as the two halves of a cyclicAMI
    /// pair do once transformed onto each other); `true` expects them to point
    /// the same way.
    pub reverse_target: bool,
    /// `lowWeightCorrection` (default `-1`, off). When `> 0` a stalled walk is
    /// not an error, and faces with a weight sum below it are counted.
    pub low_weight_correction: f64,
    /// `triMode` (default [`AmiTriangulationMode::Mesh`]).
    pub triangulation: AmiTriangulationMode,
    /// `restartUncoveredSourceFace` (default `true`).
    pub restart_uncovered_source_face: bool,
    /// `maxDistance2` `[m²]` (default `-1`, off): reject a pair whose squared
    /// face-centre distance along the source normal is at least this.
    pub max_distance2: f64,
    /// `minCosAngle` (default `-1`, off): reject a pair whose normals are less
    /// aligned than this.
    pub min_cos_angle: f64,
    /// `areaNormalisationMode` (default [`AmiAreaNormalisation::Project`]).
    pub area_normalisation: AmiAreaNormalisation,
}

impl Default for FaceAreaWeightAmiOptions {
    fn default() -> Self {
        Self {
            require_match: true,
            reverse_target: false,
            low_weight_correction: -1.0,
            triangulation: AmiTriangulationMode::Mesh,
            restart_uncovered_source_face: true,
            max_distance2: -1.0,
            min_cos_angle: -1.0,
            area_normalisation: AmiAreaNormalisation::Project,
        }
    }
}

/// A condition upstream reports with `WarningInFunction` and then continues.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmiWarning {
    /// `checkPatches`: with `requireMatch`, the source bounding box is not
    /// inside the target bounding box inflated by `0.05·|span|`.
    BoundingBoxesNotSimilar,
    /// `initialiseWalk`: source faces but no target faces; no addressing.
    NoTargetFaces,
    /// `calcInterArea` / `overlaps`: the pair's resultant normal
    /// `-n_src ± n_tgt` has zero length (typically: the patches face the same
    /// way while `reverse_target` is `false`); the pair's overlap is taken as 0.
    InvalidNormal {
        /// Source face index.
        source_face: usize,
        /// Target face index.
        target_face: usize,
    },
}

/// A condition upstream treats as `FatalError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmiError {
    /// A face refers to a point index outside its patch's `points`.
    PointIndexOutOfRange {
        /// Which patch.
        side: AmiPatchSide,
        /// Face index.
        face: usize,
        /// The offending point index.
        point: usize,
    },
    /// `face::split`: "asked to split a face with < 3 vertices"
    /// ([`AmiTriangulationMode::Mesh`] only, as upstream).
    FaceTooFewVertices {
        /// Which patch.
        side: AmiPatchSide,
        /// Face index.
        face: usize,
    },
    /// `faceAreaWeightAMI::setNextFaces`: "Unable to set target face for
    /// source face" while `mustMatchFaces()` (`requireMatch` and no
    /// `lowWeightCorrection`).
    UnableToSetTargetFace {
        /// The last source face tried.
        source_face: usize,
    },
}

impl core::fmt::Display for AmiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AmiError::PointIndexOutOfRange { side, face, point } => write!(
                f,
                "AMI {side:?} patch face {face} refers to point {point}, out of range"
            ),
            AmiError::FaceTooFewVertices { side, face } => write!(
                f,
                "AMI {side:?} patch face {face}: asked to split a face with < 3 vertices"
            ),
            AmiError::UnableToSetTargetFace { source_face } => write!(
                f,
                "AMI: unable to set target face for source face {source_face}"
            ),
        }
    }
}

impl core::error::Error for AmiError {}

/// The weights and addressing `faceAreaWeightAMI::calculate` leaves in an
/// `AMIInterpolation`, serial path.
///
/// Every `src_*` list has one entry per source face, every `tgt_*` list one
/// per target face (except when the source patch is empty: then, as upstream,
/// nothing is triangulated and `tgt_mag_sf` is empty).
#[derive(Debug, Clone, PartialEq)]
pub struct FaceAreaWeightAmiResult {
    /// `srcAddress`: for each source face, the overlapping target faces.
    pub src_address: Vec<Vec<usize>>,
    /// `srcWeights`: overlap area / `src_mag_sf` (or / Σ overlap area when
    /// `require_match`), aligned with [`src_address`](Self::src_address).
    pub src_weights: Vec<Vec<f64>>,
    /// `srcWeightsSum`: Σ overlap area / `src_mag_sf` (the covered fraction,
    /// **before** conformal rescaling), 0 for a face with no overlap.
    pub src_weights_sum: Vec<f64>,
    /// `srcCentroids`: area-weighted centroid of each overlap `[m]`.
    pub src_centroids: Vec<Vec<Vector3>>,
    /// `srcMagSf`: source face areas `[m²]`.
    pub src_mag_sf: Vec<f64>,
    /// The raw overlap areas `[m²]` aligned with `src_address` (upstream's
    /// `srcWeights` before `normaliseWeights`).
    pub src_overlap_areas: Vec<Vec<f64>>,
    /// `tgtAddress`: for each target face, the overlapping source faces.
    pub tgt_address: Vec<Vec<usize>>,
    /// `tgtWeights`, normalised as [`src_weights`](Self::src_weights).
    pub tgt_weights: Vec<Vec<f64>>,
    /// `tgtWeightsSum`.
    pub tgt_weights_sum: Vec<f64>,
    /// `tgtMagSf`: target face areas `[m²]`.
    pub tgt_mag_sf: Vec<f64>,
    /// The raw overlap areas `[m²]` aligned with `tgt_address`.
    pub tgt_overlap_areas: Vec<Vec<f64>>,
    /// `srcNonOverlap`: source faces the main walk could not process (before
    /// the restart pass, as upstream).
    pub src_non_overlap: Vec<usize>,
    /// Source faces with `src_weights_sum < low_weight_correction`.
    pub src_n_low_weight: usize,
    /// Target faces with `tgt_weights_sum < low_weight_correction`.
    pub tgt_n_low_weight: usize,
    /// Upstream warnings, in the order they were raised.
    pub warnings: Vec<AmiWarning>,
}

/// Where one AMI patch lives in an [`FvMesh`](crate::mesh::FvMesh): what
/// [`FaceAreaWeightAmiResult::to_ami_couplings`] needs to turn local face
/// indices into global faces and cells.
#[derive(Debug, Clone, PartialEq)]
pub struct AmiPatchPlacement {
    /// Index of the patch in the mesh's patch list.
    pub patch: usize,
    /// Global index of the patch's first face.
    pub start: usize,
    /// Owner cell of each local face of the patch.
    pub face_cells: Vec<usize>,
}

impl FaceAreaWeightAmiResult {
    /// Turn the weights of one side into the [`AmiCoupling`] list the
    /// finite-volume builder ([`FvMeshBuilder::ami_couplings`](crate::mesh::FvMeshBuilder::ami_couplings))
    /// consumes.
    ///
    /// `side` picks which half plays the [`AmiCoupling`] *target*: with
    /// [`AmiPatchSide::Target`] each coupling is one upstream target face, its
    /// [`AmiWeight`]s are `tgt_address`/`tgt_weights`, and `overlap_area` is
    /// the raw intersection area; [`AmiPatchSide::Source`] does the same from
    /// `src_address`/`src_weights`. `coupled` places that half in the mesh and
    /// `partner` the other half.
    ///
    /// A face with no overlap is **left out**: the builder rejects an
    /// [`AmiCoupling`] with no weights (upstream instead keeps the face and
    /// lets `lowWeightCorrection` fall back on the face's own value when
    /// interpolating, which this crate's operators do not model).
    ///
    /// # Panics
    /// Panics if a `face_cells` list is shorter than its patch.
    pub fn to_ami_couplings(
        &self,
        side: AmiPatchSide,
        coupled: &AmiPatchPlacement,
        partner: &AmiPatchPlacement,
    ) -> Vec<AmiCoupling> {
        let (address, weights, areas) = match side {
            AmiPatchSide::Target => (
                &self.tgt_address,
                &self.tgt_weights,
                &self.tgt_overlap_areas,
            ),
            AmiPatchSide::Source => (
                &self.src_address,
                &self.src_weights,
                &self.src_overlap_areas,
            ),
        };
        let mut out = Vec::with_capacity(address.len());
        for (local, addr) in address.iter().enumerate() {
            if addr.is_empty() {
                continue;
            }
            let ws = addr
                .iter()
                .zip(weights[local].iter())
                .zip(areas[local].iter())
                .map(|((&other, &w), &a)| AmiWeight {
                    source_face: partner.start + other,
                    source_cell: partner.face_cells[other],
                    weight: w,
                    overlap_area: a,
                })
                .collect();
            out.push(AmiCoupling {
                target_face: coupled.start + local,
                target_cell: coupled.face_cells[local],
                target_patch: coupled.patch,
                source_patch: partner.patch,
                local,
                weights: ws,
            });
        }
        out
    }
}

// ── OpenFOAM primitives: triangle, face, boundBox ────────────────────────────

/// `triangle::areaNormal(p0, p1, p2)` = `0.5*((p1 - p0)^(p2 - p0))`.
#[inline]
fn tri_area_normal(p0: Vector3, p1: Vector3, p2: Vector3) -> Vector3 {
    0.5 * (p1 - p0).cross(p2 - p0)
}

/// `triangle::centre(p0, p1, p2)` = `(1.0/3.0)*(p0 + p1 + p2)`.
#[inline]
fn tri_centre(p0: Vector3, p1: Vector3, p2: Vector3) -> Vector3 {
    (1.0 / 3.0) * (p0 + p1 + p2)
}

/// `triPoints::mag()`.
#[inline]
fn tri_mag(t: &Tri) -> f64 {
    tri_area_normal(t[0], t[1], t[2]).mag()
}

/// `face::fcIndex` (next index, cyclic).
#[inline]
fn fc_index(i: usize, n: usize) -> usize {
    if i + 1 == n {
        0
    } else {
        i + 1
    }
}

/// `face::rcIndex` (previous index, cyclic).
#[inline]
fn rc_index(i: usize, n: usize) -> usize {
    if i == 0 {
        n - 1
    } else {
        i - 1
    }
}

/// `face::average(points)`: arithmetic mean of the vertices.
fn face_average(f: &[usize], points: &[Vector3]) -> Vector3 {
    let mut avg = Vector3::ZERO;
    let n = f.len();
    if n > 0 {
        for &pointi in f {
            avg += points[pointi];
        }
        avg /= n as f64;
    }
    avg
}

/// `face::areaNormal(points)`: triangle directly, else the sum of the
/// triangles `(p_i, p_{i+1}, centre)` about the vertex average.
fn face_area_normal(f: &[usize], points: &[Vector3]) -> Vector3 {
    let n_points = f.len();
    if n_points == 3 {
        return tri_area_normal(points[f[0]], points[f[1]], points[f[2]]);
    }
    let centre_point = face_average(f, points);
    let mut n = Vector3::ZERO;
    for p_i in 0..n_points {
        let next_pti = if p_i == n_points - 1 { 0 } else { p_i + 1 };
        let this_point = points[f[p_i]];
        let next_point = points[f[next_pti]];
        n += tri_area_normal(this_point, next_point, centre_point);
    }
    n
}

/// `face::unitNormal(points)`: area normal normalised with `ROOTVSMALL`.
fn face_unit_normal(f: &[usize], points: &[Vector3]) -> Vector3 {
    face_area_normal(f, points).normalise(ROOTVSMALL)
}

/// `face::centre(points)`: area-weighted centre of the triangles about the
/// vertex average.
fn face_centre(f: &[usize], points: &[Vector3]) -> Vector3 {
    let n_points = f.len();
    if n_points == 3 {
        return tri_centre(points[f[0]], points[f[1]], points[f[2]]);
    }
    let centre_point = face_average(f, points);
    let mut sum_a = 0.0_f64;
    let mut sum_ac = Vector3::ZERO;
    for p_i in 0..n_points {
        let next_pti = if p_i == n_points - 1 { 0 } else { p_i + 1 };
        let this_point = points[f[p_i]];
        let next_point = points[f[next_pti]];
        // 3*triangle centre
        let ttc = this_point + next_point + centre_point;
        // 2*triangle area
        let ta = (this_point - centre_point)
            .cross(next_point - centre_point)
            .mag();
        sum_a += ta;
        sum_ac += ta * ttc;
    }
    if sum_a > VSMALL {
        sum_ac / (3.0 * sum_a)
    } else {
        centre_point
    }
}

/// `face::calcEdgeVectors`: unit vector of each edge `p_i -> p_{i+1}`.
fn face_edge_vectors(f: &[usize], points: &[Vector3]) -> Vec<Vector3> {
    let n = f.len();
    (0..n)
        .map(|i| (points[f[fc_index(i, n)]] - points[f[i]]).normalise(ROOTVSMALL))
        .collect()
}

/// `face::mostConcaveAngle`: index of the vertex with the largest internal
/// angle, and that angle.
fn face_most_concave_angle(f: &[usize], points: &[Vector3], edges: &[Vector3]) -> (usize, f64) {
    let n = face_area_normal(f, points);
    let mut index = 0;
    let mut max_angle = -GREAT;
    for i in 0..edges.len() {
        let left_edge = edges[rc_index(i, edges.len())];
        let right_edge = edges[i];
        let edge_normal = right_edge.cross(left_edge);
        // NOTE: is -ve angle since left edge pointing in other direction
        let edge_cos = left_edge.dot(right_edge);
        let edge_angle = edge_cos.max(-1.0).min(1.0).r_acos();
        let angle = if edge_normal.dot(n) > 0.0 {
            // Concave angle.
            PI + edge_angle
        } else {
            // Convex angle.
            PI - edge_angle
        };
        if max_angle < angle {
            max_angle = angle;
            index = i;
        }
    }
    (index, max_angle)
}

/// `face::split(SPLITTRIANGLE, ...)`, i.e. `face::triangles`: appends the
/// face's triangles to `tri_faces`. `Err(())` for a face with < 3 vertices.
fn face_split_triangles(
    f: &[usize],
    points: &[Vector3],
    tri_faces: &mut Vec<[usize; 3]>,
) -> Result<(), ()> {
    let size = f.len();
    if size < 3 {
        return Err(());
    } else if size == 3 {
        tri_faces.push([f[0], f[1], f[2]]);
    } else if size == 4 {
        // Start at point with largest internal angle.
        let edges = face_edge_vectors(f, points);
        let (start_index, _min_angle) = face_most_concave_angle(f, points, &edges);
        let next_index = fc_index(start_index, size);
        let split_index = fc_index(next_index, size);
        tri_faces.push([f[start_index], f[next_index], f[split_index]]);
        tri_faces.push([
            f[split_index],
            f[fc_index(split_index, size)],
            f[start_index],
        ]);
    } else {
        // General case. Like quad: search for largest internal angle.
        let edges = face_edge_vectors(f, points);
        let (start_index, min_angle) = face_most_concave_angle(f, points, &edges);
        let bisect_angle = min_angle / 2.0;
        let right_edge = edges[start_index];

        // Look for opposite point which as close as possible bisects angle;
        // split candidate starts two points away.
        let mut index = fc_index(fc_index(start_index, size), size);
        let mut min_index = index;
        let mut min_diff = PI;
        for _ in 0..(size - 3) {
            let split_edge = (points[f[index]] - points[f[start_index]]).normalise(ROOTVSMALL);
            let split_cos = split_edge.dot(right_edge);
            let split_angle = split_cos.max(-1.0).min(1.0).r_acos();
            let angle_diff = (split_angle - bisect_angle).abs();
            if angle_diff < min_diff {
                min_diff = angle_diff;
                min_index = index;
            }
            index = fc_index(index, size);
        }

        // face1: startIndex to minIndex; face2: minIndex to startIndex
        let diff = if min_index > start_index {
            min_index - start_index
        } else {
            min_index + size - start_index
        };
        let n_points1 = diff + 1;
        let n_points2 = size - diff + 1;

        let mut face1 = Vec::with_capacity(n_points1);
        index = start_index;
        for _ in 0..n_points1 {
            face1.push(f[index]);
            index = fc_index(index, size);
        }
        let mut face2 = Vec::with_capacity(n_points2);
        index = min_index;
        for _ in 0..n_points2 {
            face2.push(f[index]);
            index = fc_index(index, size);
        }
        face_split_triangles(&face1, points, tri_faces)?;
        face_split_triangles(&face2, points, tri_faces)?;
    }
    Ok(())
}

/// `faceAreaIntersect::triangleFan`.
fn face_triangle_fan(f: &[usize], faces: &mut Vec<[usize; 3]>) {
    if f.len() > 2 {
        let v0 = 0;
        for i in 1..f.len() - 1 {
            faces.push([f[v0], f[i], f[i + 1]]);
        }
    }
}

/// Distance from `p` to triangle `(a, b, c)`: the distance of
/// `triangle::nearestPointClassify` (Ericson, *Real-time collision
/// detection*, 2005, pp. 136-142), branch for branch.
fn tri_nearest_distance(a: Vector3, b: Vector3, c: Vector3, p: Vector3) -> f64 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a - p).mag();
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b - p).mag();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        if (d1 - d3) < ROOTVSMALL {
            return (a - p).mag();
        }
        let v = d1 / (d1 - d3);
        let near_pt = a + v * ab;
        return (near_pt - p).mag();
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c - p).mag();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        if (d2 - d6) < ROOTVSMALL {
            return (a - p).mag();
        }
        let w = d2 / (d2 - d6);
        let near_pt = a + w * ac;
        return (near_pt - p).mag();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        if ((d4 - d3) + (d5 - d6)) < ROOTVSMALL {
            return (b - p).mag();
        }
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let near_pt = b + w * (c - b);
        return (near_pt - p).mag();
    }
    if (va + vb + vc) < ROOTVSMALL {
        let near_pt = tri_centre(a, b, c);
        return (near_pt - p).mag();
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let near_pt = a + ab * v + ac * w;
    (near_pt - p).mag()
}

/// Distance of `face::nearestPoint(p, points)`: a triangle directly, else
/// the nearest of the triangles `(p_i, p_{i+1}, centre)`.
fn face_nearest_distance(f: &[usize], points: &[Vector3], p: Vector3) -> f64 {
    if f.len() == 3 {
        return tri_nearest_distance(points[f[0]], points[f[1]], points[f[2]], p);
    }
    let ctr = face_centre(f, points);
    let mut nearest = GREAT;
    let n_points = f.len();
    for p_i in 0..n_points {
        let next_point = points[f[fc_index(p_i, n_points)]];
        let cur = tri_nearest_distance(points[f[p_i]], next_point, ctr, p);
        if cur.abs() < nearest.abs() {
            nearest = cur;
        }
    }
    nearest
}

/// `boundBox(points, indices)`: `(min, max)` over the indexed points.
fn bound_box_of(points: &[Vector3], indices: impl Iterator<Item = usize>) -> (Vector3, Vector3) {
    // boundBox::invertedBox
    let mut min = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
    let mut max = Vector3::new(-f64::MAX, -f64::MAX, -f64::MAX);
    for i in indices {
        let p = points[i];
        min = Vector3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vector3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }
    (min, max)
}

// ── faceAreaIntersect ────────────────────────────────────────────────────────

/// `faceAreaIntersect::planeIntersection`.
#[inline]
fn plane_intersection(d: &[f64; 3], t: &Tri, neg_i: usize, pos_i: usize) -> Vector3 {
    let dp = d[pos_i];
    let dn = d[neg_i];
    (dp * t[neg_i] - dn * t[pos_i]) / (-dn + dp)
}

/// `faceAreaIntersect::setTriPoints`.
#[inline]
fn set_tri_points(a: Vector3, b: Vector3, c: Vector3, count: &mut usize, tris: &mut [Tri; 10]) {
    tris[*count] = [a, b, c];
    *count += 1;
}

/// `faceAreaIntersect::triSliceWithPlane`: keep the part of `tri` on the
/// positive side of the plane `(origin, normal)`.
fn tri_slice_with_plane(
    tri: &Tri,
    origin: Vector3,
    normal: Vector3,
    tris: &mut [Tri; 10],
    n_tris: &mut usize,
    len: f64,
) {
    let mut d = [0.0_f64; 3];
    let mut n_co_planar = 0;
    let mut n_pos = 0;
    let mut pos_i = usize::MAX;
    let mut neg_i = usize::MAX;
    let mut cop_i = usize::MAX;
    for i in 0..3 {
        d[i] = (tri[i] - origin).dot(normal);
        if d[i].abs() < FACE_AREA_INTERSECT_TOLERANCE * len {
            n_co_planar += 1;
            cop_i = i;
            d[i] = 0.0;
        } else if d[i] > 0.0 {
            n_pos += 1;
            pos_i = i;
        } else {
            neg_i = i;
        }
    }

    if (n_pos == 3) || ((n_pos == 2) && (n_co_planar == 1)) || ((n_pos == 1) && (n_co_planar == 2))
    {
        // all points above cutting plane - add complete triangle
        tris[*n_tris] = *tri;
        *n_tris += 1;
    } else if (n_pos == 2) && (n_co_planar == 0) {
        // 2 points above plane, 1 below: quad above split into 2 triangles
        let i0 = neg_i;
        let i1 = fc_index(i0, 3);
        let i2 = fc_index(i1, 3);
        let p01 = plane_intersection(&d, tri, i0, i1);
        let p02 = plane_intersection(&d, tri, i0, i2);
        set_tri_points(tri[i1], tri[i2], p02, n_tris, tris);
        set_tri_points(tri[i1], p02, p01, n_tris, tris);
    } else if n_pos == 1 {
        let i0 = pos_i;
        if n_co_planar == 0 {
            // 1 point above plane, 2 below: keep triangle above
            let i1 = fc_index(i0, 3);
            let i2 = fc_index(i1, 3);
            let p01 = plane_intersection(&d, tri, i1, i0);
            let p02 = plane_intersection(&d, tri, i2, i0);
            set_tri_points(tri[i0], p01, p02, n_tris, tris);
        } else {
            // 1 above, 1 on, 1 below: keep triangle above
            let i1 = neg_i;
            let i2 = cop_i;
            let p01 = plane_intersection(&d, tri, i1, i0);
            if fc_index(i0, 3) == i1 {
                set_tri_points(tri[i0], p01, tri[i2], n_tris, tris);
            } else {
                set_tri_points(tri[i0], tri[i2], p01, n_tris, tris);
            }
        }
    }
    // else: all points below cutting plane - forget
}

/// One target-edge cut of `faceAreaIntersect::triangleIntersect`: the plane
/// through `origin` with normal `(edge ^ (-s*n))/|..|`. `None` when upstream
/// returns early (zero edge length or edge parallel to `n`).
#[inline]
fn edge_cut_plane(edge: Vector3, s: f64, n: Vector3) -> Option<Vector3> {
    if s < ROOTVSMALL {
        return None;
    }
    // Note: outer product with n pre-scaled with edge length (upstream).
    let n0 = edge.cross(-s * n);
    let mag_sqr_n0 = n0.mag_sqr();
    if mag_sqr_n0 < ROOTVSMALL {
        None
    } else {
        Some(n0 / mag_sqr_n0.sqrt())
    }
}

/// `faceAreaIntersect::triangleIntersect`: add the area (and area-weighted
/// centroid) of the part of `src` inside the prism of target triangle
/// `(tgt0, tgt1, tgt2)` along `n`.
fn triangle_intersect(
    src: &Tri,
    tgt0: Vector3,
    tgt1: Vector3,
    tgt2: Vector3,
    n: Vector3,
    area: &mut f64,
    centroid: &mut Vector3,
) {
    let zero_tri = [Vector3::ZERO; 3];
    let mut work_tris1 = [zero_tri; 10];
    let mut n_work_tris1 = 0;
    let mut work_tris2 = [zero_tri; 10];
    let mut n_work_tris2 = 0;

    let src_area = tri_mag(src);
    if src_area < ROOTVSMALL {
        return;
    }
    // Typical length scale
    let t = src_area.sqrt();

    // Edge 0
    let Some(pl0) = edge_cut_plane(tgt0 - tgt1, (tgt1 - tgt0).mag(), n) else {
        return;
    };
    tri_slice_with_plane(src, tgt0, pl0, &mut work_tris1, &mut n_work_tris1, t);
    if n_work_tris1 == 0 {
        return;
    }

    // Edge 1
    let Some(pl1) = edge_cut_plane(tgt1 - tgt2, (tgt2 - tgt1).mag(), n) else {
        return;
    };
    for i in 0..n_work_tris1 {
        let tri = work_tris1[i];
        tri_slice_with_plane(&tri, tgt1, pl1, &mut work_tris2, &mut n_work_tris2, t);
    }
    if n_work_tris2 == 0 {
        return;
    }

    // Edge 2
    let Some(pl2) = edge_cut_plane(tgt2 - tgt0, (tgt2 - tgt0).mag(), n) else {
        return;
    };
    n_work_tris1 = 0;
    for i in 0..n_work_tris2 {
        let tri = work_tris2[i];
        tri_slice_with_plane(&tri, tgt2, pl2, &mut work_tris1, &mut n_work_tris1, t);
    }
    for tri in work_tris1.iter().take(n_work_tris1) {
        let curr_area = tri_mag(tri);
        *area += curr_area;
        *centroid += curr_area * tri_centre(tri[0], tri[1], tri[2]);
    }
}

/// Target triangle vertices in the order `faceAreaIntersect::calc` passes
/// them: as stored when `reverse_b`, reversed otherwise.
#[inline]
fn target_tri_points(points_b: &[Vector3], tri_b: &[usize; 3], reverse_b: bool) -> Tri {
    if reverse_b {
        [points_b[tri_b[0]], points_b[tri_b[1]], points_b[tri_b[2]]]
    } else {
        [points_b[tri_b[2]], points_b[tri_b[1]], points_b[tri_b[0]]]
    }
}

/// `faceAreaIntersect::calc`: intersection area of face A (as triangles
/// `tris_a`) with face B (as `tris_b`) along `n`, and its centroid.
fn face_area_intersect_calc(
    points_a: &[Vector3],
    points_b: &[Vector3],
    tris_a: &[[usize; 3]],
    tris_b: &[[usize; 3]],
    reverse_b: bool,
    n: Vector3,
) -> (f64, Vector3) {
    let mut area = 0.0;
    let mut centroid = Vector3::ZERO;
    for tri_a in tris_a {
        let tp_a = [points_a[tri_a[0]], points_a[tri_a[1]], points_a[tri_a[2]]];
        for tri_b in tris_b {
            let tb = target_tri_points(points_b, tri_b, reverse_b);
            triangle_intersect(&tp_a, tb[0], tb[1], tb[2], n, &mut area, &mut centroid);
        }
    }
    if area > 0.0 {
        centroid /= area;
    }
    (area, centroid)
}

/// `faceAreaIntersect::overlaps`: whether the accumulated intersection area
/// exceeds `threshold` (checked after every target triangle, as upstream).
fn face_area_intersect_overlaps(
    points_a: &[Vector3],
    points_b: &[Vector3],
    tris_a: &[[usize; 3]],
    tris_b: &[[usize; 3]],
    reverse_b: bool,
    n: Vector3,
    threshold: f64,
) -> bool {
    let mut area = 0.0;
    let mut centroid = Vector3::ZERO;
    for tri_a in tris_a {
        let tp_a = [points_a[tri_a[0]], points_a[tri_a[1]], points_a[tri_a[2]]];
        for tri_b in tris_b {
            let tb = target_tri_points(points_b, tri_b, reverse_b);
            triangle_intersect(&tp_a, tb[0], tb[1], tb[2], n, &mut area, &mut centroid);
            if area > threshold {
                return true;
            }
        }
    }
    false
}

// ── primitivePatch and advancingFrontAMI ─────────────────────────────────────

/// `PrimitivePatch::calcAddressing`'s `faceFaces`: edge-neighbour faces, in
/// upstream's order (lower-numbered neighbours in the order they were
/// processed, then higher-numbered ones by this face's edge order).
fn patch_face_faces(faces: &[Vec<usize>], n_points: usize) -> Vec<Vec<usize>> {
    // calcPointFaces: faces of each point, ascending.
    let mut pf: Vec<Vec<usize>> = vec![Vec::new(); n_points];
    for (facei, f) in faces.iter().enumerate() {
        for &pointi in f {
            pf[pointi].push(facei);
        }
    }
    let face_into_edges: Vec<Vec<(usize, usize)>> = faces
        .iter()
        .map(|f| {
            (0..f.len())
                .map(|i| (f[i], f[fc_index(i, f.len())]))
                .collect()
        })
        .collect();
    let edge_eq = |a: (usize, usize), b: (usize, usize)| {
        (a.0 == b.0 && a.1 == b.1) || (a.0 == b.1 && a.1 == b.0)
    };
    let mut face_edges: Vec<Vec<bool>> = faces.iter().map(|f| vec![false; f.len()]).collect();
    let mut ff: Vec<Vec<usize>> = vec![Vec::new(); faces.len()];

    for facei in 0..faces.len() {
        let n_cur = faces[facei].len();
        let mut nei_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cur];
        let mut edge_of_nei_face: Vec<Vec<usize>> = vec![Vec::new(); n_cur];
        let mut n_neighbours = 0;
        for edge_i in 0..n_cur {
            if face_edges[facei][edge_i] {
                continue;
            }
            let e = face_into_edges[facei][edge_i];
            let mut found = false;
            for &cur_nei in &pf[e.0] {
                if cur_nei > facei {
                    for (nei_edge_i, &se) in face_into_edges[cur_nei].iter().enumerate() {
                        if edge_eq(se, e) {
                            found = true;
                            nei_faces[edge_i].push(cur_nei);
                            edge_of_nei_face[edge_i].push(nei_edge_i);
                            ff[facei].push(cur_nei);
                            ff[cur_nei].push(facei);
                        }
                    }
                }
            }
            if found {
                n_neighbours += 1;
            }
        }
        for _ in 0..n_neighbours {
            let mut next_nei = None;
            let mut min_nei = faces.len();
            for (nf_i, nf) in nei_faces.iter().enumerate() {
                if !nf.is_empty() && nf[0] < min_nei {
                    next_nei = Some(nf_i);
                    min_nei = nf[0];
                }
            }
            // Upstream: FatalError "Error in internal edge insertion" when
            // none is found; unreachable since n_neighbours counts them.
            if let Some(nn) = next_nei {
                face_edges[facei][nn] = true;
                for (k, &c) in nei_faces[nn].iter().enumerate() {
                    face_edges[c][edge_of_nei_face[nn][k]] = true;
                }
                nei_faces[nn].clear();
                edge_of_nei_face[nn].clear();
            }
        }
    }
    ff
}

/// One patch with the derived data `advancingFrontAMI` uses.
#[derive(Debug, Clone)]
struct AmiPatchData {
    points: Vec<Vector3>,
    faces: Vec<Vec<usize>>,
    face_centres: Vec<Vector3>,
    face_normals: Vec<Vector3>,
    face_faces: Vec<Vec<usize>>,
    tris: Vec<Vec<[usize; 3]>>,
    mag_sf: Vec<f64>,
}

impl AmiPatchData {
    /// Derived data, plus `advancingFrontAMI::triangulatePatch`.
    fn new(
        geom: &AmiPatchGeometry,
        side: AmiPatchSide,
        tri_mode: AmiTriangulationMode,
        normalisation: AmiAreaNormalisation,
    ) -> Result<Self, AmiError> {
        for (face, f) in geom.faces.iter().enumerate() {
            for &point in f {
                if point >= geom.points.len() {
                    return Err(AmiError::PointIndexOutOfRange { side, face, point });
                }
            }
        }
        let points = &geom.points;
        let face_centres = geom.faces.iter().map(|f| face_centre(f, points)).collect();
        let face_normals: Vec<Vector3> = geom
            .faces
            .iter()
            .map(|f| face_unit_normal(f, points))
            .collect();
        let face_faces = patch_face_faces(&geom.faces, points.len());

        let mut tris = Vec::with_capacity(geom.faces.len());
        let mut mag_sf = Vec::with_capacity(geom.faces.len());
        for (facei, f) in geom.faces.iter().enumerate() {
            let mut tri_faces = Vec::new();
            match tri_mode {
                AmiTriangulationMode::Fan => face_triangle_fan(f, &mut tri_faces),
                AmiTriangulationMode::Mesh => face_split_triangles(f, points, &mut tri_faces)
                    .map_err(|()| AmiError::FaceTooFewVertices { side, face: facei })?,
            }
            let mut m = 0.0;
            match normalisation {
                AmiAreaNormalisation::Project => {
                    for t in &tri_faces {
                        m += tri_area_normal(points[t[0]], points[t[1]], points[t[2]])
                            .dot(face_normals[facei]);
                    }
                }
                AmiAreaNormalisation::Mag => {
                    for t in &tri_faces {
                        m += tri_area_normal(points[t[0]], points[t[1]], points[t[2]]).mag();
                    }
                }
            }
            tris.push(tri_faces);
            mag_sf.push(m);
        }
        Ok(Self {
            points: geom.points.clone(),
            faces: geom.faces.clone(),
            face_centres,
            face_normals,
            face_faces,
            tris,
            mag_sf,
        })
    }

    /// `meshPoints()` bounding box.
    fn bound_box(&self) -> (Vector3, Vector3) {
        bound_box_of(&self.points, self.faces.iter().flatten().copied())
    }
}

/// Per-source / per-target working lists of `faceAreaWeightAMI::calculate`.
#[derive(Debug, Clone)]
struct AmiLists {
    src_addr: Vec<Vec<usize>>,
    src_wght: Vec<Vec<f64>>,
    src_ctr: Vec<Vec<Vector3>>,
    tgt_addr: Vec<Vec<usize>>,
    tgt_wght: Vec<Vec<f64>>,
}

/// The serial `faceAreaWeightAMI` state.
#[derive(Debug, Clone)]
struct FaceAreaWeightAmi {
    src: AmiPatchData,
    tgt: AmiPatchData,
    opts: FaceAreaWeightAmiOptions,
    /// `cos(degToRad(89))` of `appendNbrFaces`.
    theta_cos: f64,
    src_non_overlap: Vec<usize>,
    warnings: Vec<AmiWarning>,
}

impl FaceAreaWeightAmi {
    /// `AMIInterpolation::mustMatchFaces`.
    fn must_match_faces(&self) -> bool {
        self.opts.require_match && !(self.opts.low_weight_correction > 0.0)
    }

    /// `advancingFrontAMI::checkPatches` (serial: `comm() != -1`).
    fn check_patches(&mut self) {
        if self.opts.require_match {
            let (src_min, src_max) = self.src.bound_box();
            let (tgt_min, tgt_max) = self.tgt.bound_box();
            // boundBox::inflate(s) = grow(s*mag())
            let delta = MAX_BOUNDS_ERROR * tgt_min.dist(tgt_max);
            let inf_min = Vector3::new(tgt_min.x - delta, tgt_min.y - delta, tgt_min.z - delta);
            let inf_max = Vector3::new(tgt_max.x + delta, tgt_max.y + delta, tgt_max.z + delta);
            let contains = |p: Vector3| {
                inf_min.x <= p.x
                    && p.x <= inf_max.x
                    && inf_min.y <= p.y
                    && p.y <= inf_max.y
                    && inf_min.z <= p.z
                    && p.z <= inf_max.z
            };
            if !(contains(src_min) && contains(src_max)) {
                self.warnings.push(AmiWarning::BoundingBoxesNotSimilar);
            }
        }
    }

    /// `advancingFrontAMI::isCandidate`.
    fn is_candidate(&self, src_facei: usize, tgt_facei: usize) -> bool {
        if self.src.mag_sf[src_facei] < ROOTVSMALL || self.tgt.mag_sf[tgt_facei] < ROOTVSMALL {
            return false;
        }
        if self.opts.max_distance2 > 0.0 {
            let src_fc = self.src.face_centres[src_facei];
            let tgt_fc = self.tgt.face_centres[tgt_facei];
            let src_n = self.src.face_normals[src_facei];
            let normal_dist = (tgt_fc - src_fc).dot(src_n);
            if normal_dist * normal_dist >= self.opts.max_distance2 {
                return false;
            }
        }
        if self.opts.min_cos_angle > -1.0 {
            let src_n = self.src.face_normals[src_facei];
            let mut tgt_n = self.tgt.face_normals[tgt_facei];
            if !self.opts.reverse_target {
                tgt_n = -tgt_n;
            }
            if src_n.dot(tgt_n) <= self.opts.min_cos_angle {
                return false;
            }
        }
        true
    }

    /// The "crude resultant norm" of `calcInterArea` / `overlaps`, or `None`
    /// (with the upstream warning) when it has zero length.
    fn resultant_normal(&mut self, src_facei: usize, tgt_facei: usize) -> Option<Vector3> {
        let mut n = -self.src.face_normals[src_facei];
        if self.opts.reverse_target {
            n -= self.tgt.face_normals[tgt_facei];
        } else {
            n += self.tgt.face_normals[tgt_facei];
        }
        let mag_n = n.mag();
        if mag_n > ROOTVSMALL {
            Some(n / mag_n)
        } else {
            self.warnings.push(AmiWarning::InvalidNormal {
                source_face: src_facei,
                target_face: tgt_facei,
            });
            None
        }
    }

    /// `faceAreaWeightAMI::calcInterArea`: `(area, centroid)`, zero when the
    /// pair is rejected.
    fn calc_inter_area(&mut self, src_facei: usize, tgt_facei: usize) -> (f64, Vector3) {
        if !self.is_candidate(src_facei, tgt_facei) {
            return (0.0, Vector3::ZERO);
        }
        match self.resultant_normal(src_facei, tgt_facei) {
            Some(n) => face_area_intersect_calc(
                &self.src.points,
                &self.tgt.points,
                &self.src.tris[src_facei],
                &self.tgt.tris[tgt_facei],
                self.opts.reverse_target,
                n,
            ),
            None => (0.0, Vector3::ZERO),
        }
    }

    /// `faceAreaWeightAMI::overlaps`.
    fn overlaps(&mut self, src_facei: usize, tgt_facei: usize, threshold: f64) -> bool {
        if !self.is_candidate(src_facei, tgt_facei) {
            return false;
        }
        match self.resultant_normal(src_facei, tgt_facei) {
            Some(n) => face_area_intersect_overlaps(
                &self.src.points,
                &self.tgt.points,
                &self.src.tris[src_facei],
                &self.tgt.tris[tgt_facei],
                self.opts.reverse_target,
                n,
                threshold,
            ),
            None => false,
        }
    }

    /// `advancingFrontAMI::appendNbrFaces` on the target patch.
    fn append_nbr_faces(&self, facei: usize, visited_faces: &[usize], face_ids: &mut Vec<usize>) {
        let patch = &self.tgt;
        for &nbr_facei in &patch.face_faces[facei] {
            // Prevent addition of face if it is not on the same plane-ish
            if !visited_faces.contains(&nbr_facei) && !face_ids.contains(&nbr_facei) {
                let n1 = patch.face_normals[facei];
                let n2 = patch.face_normals[nbr_facei];
                let cos_i = n1.dot(n2);
                if cos_i > self.theta_cos {
                    face_ids.push(nbr_facei);
                }
            }
        }
    }

    /// `advancingFrontAMI::findTargetFace`, with the octree replaced by a
    /// brute-force search over `findNearestMaskedOp` (see the module docs).
    fn find_target_face(
        &self,
        src_facei: usize,
        exclude_faces: &[usize],
        src_face_pti: Option<usize>,
    ) -> Option<usize> {
        let src_pts = &self.src.points;
        let src_face = &self.src.faces[src_facei];
        let (bb_min, bb_max) = bound_box_of(src_pts, src_face.iter().copied());
        let src_pt = match src_face_pti {
            None => 0.5 * (bb_min + bb_max),
            Some(fpi) => src_pts[src_face[fpi]],
        };
        let mut sample =
            self.nearest_target_face(src_pt, 0.25 * bb_min.dist_sqr(bb_max), exclude_faces);
        if sample.is_none() {
            // Fall-back for extreme cases. Should only occur sparsely
            sample = self.nearest_target_face(src_pt, GREAT * GREAT, exclude_faces);
        }
        match sample {
            Some(t) if self.is_candidate(src_facei, t) => Some(t),
            _ => None,
        }
    }

    /// `indexedOctree::findNearest(sample, startDistSqr, findNearestMaskedOp)`
    /// by brute force: the non-excluded target face strictly nearer than
    /// `sqrt(start_dist_sqr)`, the lowest index on an exact tie.
    fn nearest_target_face(
        &self,
        sample: Vector3,
        start_dist_sqr: f64,
        exclude: &[usize],
    ) -> Option<usize> {
        let mut nearest_dist_sqr = start_dist_sqr;
        let mut min_index = None;
        for (index, f) in self.tgt.faces.iter().enumerate() {
            if !exclude.contains(&index) {
                let d = face_nearest_distance(f, &self.tgt.points, sample);
                let dist_sqr = d * d;
                if dist_sqr < nearest_dist_sqr {
                    nearest_dist_sqr = dist_sqr;
                    min_index = Some(index);
                }
            }
        }
        min_index
    }

    /// `faceAreaWeightAMI::processSourceFace`.
    fn process_source_face(
        &mut self,
        src_facei: usize,
        tgt_start_facei: Option<usize>,
        nbr_faces: &mut Vec<usize>,
        visited_faces: &mut Vec<usize>,
        lists: &mut AmiLists,
    ) -> bool {
        let Some(tgt_start_facei) = tgt_start_facei else {
            return false;
        };
        nbr_faces.push(tgt_start_facei);
        self.append_nbr_faces(tgt_start_facei, visited_faces, nbr_faces);

        let mut face_processed = false;
        while let Some(tgt_facei) = nbr_faces.pop() {
            // Process new target face as LIFO
            visited_faces.push(tgt_facei);
            let (inter_area, inter_centroid) = self.calc_inter_area(src_facei, tgt_facei);
            // store when intersection fractional area > tolerance
            if inter_area / self.src.mag_sf[src_facei] > FACE_AREA_INTERSECT_TOLERANCE {
                lists.src_addr[src_facei].push(tgt_facei);
                lists.src_wght[src_facei].push(inter_area);
                lists.src_ctr[src_facei].push(inter_centroid);
                lists.tgt_addr[tgt_facei].push(src_facei);
                lists.tgt_wght[tgt_facei].push(inter_area);
                self.append_nbr_faces(tgt_facei, visited_faces, nbr_faces);
                face_processed = true;
            }
        }
        face_processed
    }

    /// `faceAreaWeightAMI::setNextFaces`. `Ok(false)` ends the walk.
    #[allow(clippy::too_many_arguments)]
    fn set_next_faces(
        &mut self,
        start_seedi: &mut usize,
        src_facei: &mut usize,
        tgt_facei: &mut Option<usize>,
        map_flag: &[bool],
        seed_faces: &mut [Option<usize>],
        visited_faces: &[usize],
        error_on_not_found: bool,
    ) -> Result<bool, AmiError> {
        if !map_flag.iter().any(|&b| b) {
            // No more faces to map
            return Ok(false);
        }
        // bitSet::find_next: first set bit strictly after `pos`.
        let find_next = |pos: usize| (pos + 1..map_flag.len()).find(|&i| map_flag[i]);

        let src_nbr_faces = self.src.face_faces[*src_facei].clone();
        *tgt_facei = None;

        // Set possible seeds for later use
        let mut values_set = false;
        for face_s in src_nbr_faces {
            if map_flag[face_s] && seed_faces[face_s].is_none() {
                for &face_t in visited_faces {
                    let threshold = self.src.mag_sf[face_s] * FACE_AREA_INTERSECT_TOLERANCE;
                    // Store when intersection fractional area > threshold
                    if self.overlaps(face_s, face_t, threshold) {
                        seed_faces[face_s] = Some(face_t);
                        if !values_set {
                            *src_facei = face_s;
                            *tgt_facei = Some(face_t);
                            values_set = true;
                        }
                    }
                }
            }
        }
        if values_set {
            return Ok(true);
        }

        // Set next src and tgt faces if not set above - try to use existing seed
        let mut facei = Some(*start_seedi);
        if !map_flag[*start_seedi] {
            facei = find_next(*start_seedi);
        }
        let start_seedi0 = facei;
        let mut found_next_seed = false;
        while let Some(fi) = facei {
            if !found_next_seed {
                *start_seedi = fi;
                found_next_seed = true;
            }
            if let Some(seed) = seed_faces[fi] {
                *src_facei = fi;
                *tgt_facei = Some(seed);
                return Ok(true);
            }
            facei = find_next(fi);
        }

        // Perform new search to find match
        facei = start_seedi0;
        while let Some(fi) = facei {
            *src_facei = fi;
            *tgt_facei = self.find_target_face(fi, visited_faces, None);
            if tgt_facei.is_some() {
                return Ok(true);
            }
            facei = find_next(fi);
        }

        if error_on_not_found {
            return Err(AmiError::UnableToSetTargetFace {
                source_face: *src_facei,
            });
        }
        Ok(false)
    }

    /// `faceAreaWeightAMI::calcAddressing`.
    fn calc_addressing(
        &mut self,
        lists: &mut AmiLists,
        mut src_facei: usize,
        tgt_facei: usize,
    ) -> Result<(), AmiError> {
        let n_faces_remaining = lists.src_addr.len();
        let mut nbr_faces: Vec<usize> = Vec::with_capacity(10);
        let mut visited_faces: Vec<usize> = Vec::with_capacity(10);
        let mut seed_faces: Vec<Option<usize>> = vec![None; n_faces_remaining];
        seed_faces[src_facei] = Some(tgt_facei);
        let mut map_flag = vec![true; n_faces_remaining];
        let mut start_seedi = 0;
        let must_match = self.must_match_faces();
        let mut non_overlap_faces = Vec::new();
        let mut tgt_facei = Some(tgt_facei);

        loop {
            nbr_faces.clear();
            visited_faces.clear();
            let face_processed = self.process_source_face(
                src_facei,
                tgt_facei,
                &mut nbr_faces,
                &mut visited_faces,
                lists,
            );
            map_flag[src_facei] = false;
            if !face_processed {
                non_overlap_faces.push(src_facei);
            }
            let continue_walk = self.set_next_faces(
                &mut start_seedi,
                &mut src_facei,
                &mut tgt_facei,
                &map_flag,
                &mut seed_faces,
                &visited_faces,
                must_match,
            )?;
            if !continue_walk {
                break;
            }
        }
        self.src_non_overlap = non_overlap_faces;
        Ok(())
    }

    /// `faceAreaWeightAMI::restartUncoveredSourceFace`.
    fn restart_uncovered_source_face(&mut self, lists: &mut AmiLists) {
        let mut nbr_faces: Vec<usize> = Vec::with_capacity(10);
        let mut visited_faces: Vec<usize>;
        for src_facei in 0..lists.src_wght.len() {
            let mut s = 0.0;
            for &w in &lists.src_wght[src_facei] {
                s += w;
            }
            let t = s / self.src.mag_sf[src_facei];
            if t < RESTART_MIN_WEIGHT {
                for fpi in 0..self.src.faces[src_facei].len() {
                    let tgt_facei =
                        self.find_target_face(src_facei, &lists.src_addr[src_facei], Some(fpi));
                    if tgt_facei.is_some() {
                        nbr_faces.clear();
                        visited_faces = lists.src_addr[src_facei].clone();
                        let _ = self.process_source_face(
                            src_facei,
                            tgt_facei,
                            &mut nbr_faces,
                            &mut visited_faces,
                            lists,
                        );
                    }
                }
            }
        }
    }
}

/// `AMIInterpolation::normaliseWeights` for one side: returns the weight sums
/// and the number of faces whose sum is below `low_weight_tol`.
fn normalise_weights(
    patch_areas: &[f64],
    wght: &mut [Vec<f64>],
    conformal: bool,
    low_weight_tol: f64,
) -> (Vec<f64>, usize) {
    let mut wght_sum = vec![0.0; wght.len()];
    let mut n_low_weight = 0;
    for (facei, w) in wght.iter_mut().enumerate() {
        if !w.is_empty() {
            let mut denom = patch_areas[facei];
            let mut s = 0.0;
            for &wi in w.iter() {
                s += wi;
            }
            let t = s / denom;
            if conformal {
                denom = s;
            }
            for wi in w.iter_mut() {
                *wi /= denom;
            }
            wght_sum[facei] = t;
            if t < low_weight_tol {
                n_low_weight += 1;
            }
        } else {
            wght_sum[facei] = 0.0;
        }
    }
    (wght_sum, n_low_weight)
}

/// Compute AMI addressing and weights between two patches by OpenFOAM ESI's
/// `faceAreaWeightAMI` (serial path): a port of
/// `faceAreaWeightAMI::calculate` followed by
/// `normaliseWeights(requireMatch, true)`. See the [module docs](self) for
/// the method and what is not ported.
///
/// The patches must coincide geometrically (the caller applies any cyclic
/// transform first) and, with the default `reverse_target = false`, face each
/// other: the target's face normals opposite to the source's.
///
/// # Errors
/// [`AmiError`] for the conditions upstream treats as fatal.
///
/// # Example
/// ```
/// use outram_foam_basic_lib::mesh::face_area_weight_ami::{
///     face_area_weight_ami, AmiPatchGeometry, FaceAreaWeightAmiOptions,
/// };
/// use outram_foam_basic_lib::primitives::Vector3;
/// // One unit square (normal +z) against two half squares facing it (-z).
/// let p = |x: f64, y: f64| Vector3::new(x, y, 0.0);
/// let src = AmiPatchGeometry {
///     points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
///     faces: vec![vec![0, 1, 2, 3]],
/// };
/// let tgt = AmiPatchGeometry {
///     points: vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(0.5, 1.0), p(1.0, 1.0)],
///     faces: vec![vec![0, 3, 4, 1], vec![1, 4, 5, 2]],
/// };
/// let ami = face_area_weight_ami(&src, &tgt, &FaceAreaWeightAmiOptions::default()).unwrap();
/// assert_eq!(ami.src_address[0].len(), 2);
/// assert!((ami.src_weights[0][0] - 0.5).abs() < 1e-12);
/// assert!((ami.src_weights_sum[0] - 1.0).abs() < 1e-12);
/// assert!((ami.tgt_weights[1][0] - 1.0).abs() < 1e-12);
/// ```
pub fn face_area_weight_ami(
    source: &AmiPatchGeometry,
    target: &AmiPatchGeometry,
    options: &FaceAreaWeightAmiOptions,
) -> Result<FaceAreaWeightAmiResult, AmiError> {
    let n_src = source.faces.len();
    let n_tgt = target.faces.len();

    // AMIInterpolation::calculate: no source faces -> no addressing, and
    // advancingFrontAMI::calculate returns before triangulating.
    if n_src == 0 {
        return Ok(FaceAreaWeightAmiResult {
            src_address: Vec::new(),
            src_weights: Vec::new(),
            src_weights_sum: Vec::new(),
            src_centroids: Vec::new(),
            src_mag_sf: Vec::new(),
            src_overlap_areas: Vec::new(),
            tgt_address: vec![Vec::new(); n_tgt],
            tgt_weights: vec![Vec::new(); n_tgt],
            tgt_weights_sum: vec![0.0; n_tgt],
            tgt_mag_sf: Vec::new(),
            tgt_overlap_areas: vec![Vec::new(); n_tgt],
            src_non_overlap: Vec::new(),
            src_n_low_weight: 0,
            tgt_n_low_weight: 0,
            warnings: Vec::new(),
        });
    }

    let src = AmiPatchData::new(
        source,
        AmiPatchSide::Source,
        options.triangulation,
        options.area_normalisation,
    )?;
    let tgt = AmiPatchData::new(
        target,
        AmiPatchSide::Target,
        options.triangulation,
        options.area_normalisation,
    )?;
    let mut ami = FaceAreaWeightAmi {
        src,
        tgt,
        opts: *options,
        theta_cos: (89.0 * PI / 180.0).r_cos(),
        src_non_overlap: Vec::new(),
        warnings: Vec::new(),
    };
    ami.check_patches();

    let mut lists = AmiLists {
        src_addr: vec![Vec::new(); n_src],
        src_wght: vec![Vec::new(); n_src],
        src_ctr: vec![Vec::new(); n_src],
        tgt_addr: vec![Vec::new(); n_tgt],
        tgt_wght: vec![Vec::new(); n_tgt],
    };

    // advancingFrontAMI::initialiseWalk(srcFacei = 0, tgtFacei = 0): with both
    // seeds already set (not -1) upstream performs no search, so the walk
    // always starts from source face 0 against target face 0.
    let ok = if n_tgt == 0 {
        ami.warnings.push(AmiWarning::NoTargetFaces);
        false
    } else {
        true
    };

    if ok {
        ami.calc_addressing(&mut lists, 0, 0)?;
        if options.restart_uncovered_source_face {
            ami.restart_uncovered_source_face(&mut lists);
        }
    }

    let src_overlap_areas = lists.src_wght.clone();
    let tgt_overlap_areas = lists.tgt_wght.clone();
    let (src_weights_sum, src_n_low_weight) = normalise_weights(
        &ami.src.mag_sf,
        &mut lists.src_wght,
        options.require_match,
        options.low_weight_correction,
    );
    let (tgt_weights_sum, tgt_n_low_weight) = normalise_weights(
        &ami.tgt.mag_sf,
        &mut lists.tgt_wght,
        options.require_match,
        options.low_weight_correction,
    );

    Ok(FaceAreaWeightAmiResult {
        src_address: lists.src_addr,
        src_weights: lists.src_wght,
        src_weights_sum,
        src_centroids: lists.src_ctr,
        src_mag_sf: ami.src.mag_sf,
        src_overlap_areas,
        tgt_address: lists.tgt_addr,
        tgt_weights: lists.tgt_wght,
        tgt_weights_sum,
        tgt_mag_sf: ami.tgt.mag_sf,
        tgt_overlap_areas,
        src_non_overlap: ami.src_non_overlap,
        src_n_low_weight,
        tgt_n_low_weight,
        warnings: ami.warnings,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::ami::overlap_weights_1d;
    use crate::mesh::FvMesh;

    /// A structured quad grid in the plane `z = 0` with vertices at
    /// `(xs[i], ys[j])`; normal `+z`, or `-z` when `flip`.
    fn grid_patch(xs: &[f64], ys: &[f64], flip: bool) -> AmiPatchGeometry {
        let nx = xs.len();
        let mut points = Vec::new();
        for &y in ys {
            for &x in xs {
                points.push(Vector3::new(x, y, 0.0));
            }
        }
        let mut faces = Vec::new();
        for j in 0..ys.len() - 1 {
            for i in 0..nx - 1 {
                let v = |a: usize, b: usize| b * nx + a;
                let mut f = vec![v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)];
                if flip {
                    f.reverse();
                }
                faces.push(f);
            }
        }
        AmiPatchGeometry { points, faces }
    }

    /// Deterministic in-plane perturbation of every interior vertex of a grid
    /// patch by up to `amp` times the local spacing (a 64-bit LCG).
    fn perturb_interior(p: &mut AmiPatchGeometry, xs: &[f64], ys: &[f64], amp: f64, seed: u64) {
        let mut s = seed;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 11) as f64 / (1u64 << 53) as f64) - 0.5
        };
        let nx = xs.len();
        for j in 1..ys.len() - 1 {
            for i in 1..nx - 1 {
                let hx = (xs[i + 1] - xs[i - 1]) * 0.5;
                let hy = (ys[j + 1] - ys[j - 1]) * 0.5;
                let k = j * nx + i;
                p.points[k].x += amp * hx * next();
                p.points[k].y += amp * hy * next();
            }
        }
    }

    /// A cylinder band of radius `r`, `0 <= z <= h`: `n_theta` flat facets
    /// (vertices on the cylinder at `theta0 + 2*pi*j/n_theta`) by `n_z` rows.
    /// Normals point outward, or inward when `inward`.
    fn cylinder_band(
        n_theta: usize,
        n_z: usize,
        r: f64,
        h: f64,
        theta0: f64,
        inward: bool,
    ) -> AmiPatchGeometry {
        let mut points = Vec::new();
        for k in 0..=n_z {
            let z = h * k as f64 / n_z as f64;
            for j in 0..n_theta {
                let th = theta0 + 2.0 * PI * j as f64 / n_theta as f64;
                points.push(Vector3::new(r * th.r_cos(), r * th.r_sin(), z));
            }
        }
        let v = |j: usize, k: usize| k * n_theta + (j % n_theta);
        let mut faces = Vec::new();
        for k in 0..n_z {
            for j in 0..n_theta {
                let mut f = vec![v(j, k), v(j + 1, k), v(j + 1, k + 1), v(j, k + 1)];
                if inward {
                    f.reverse();
                }
                faces.push(f);
            }
        }
        AmiPatchGeometry { points, faces }
    }

    fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
        (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
    }

    /// `(address, value)` pairs of one face, sorted by address.
    fn sorted_pairs(addr: &[usize], vals: &[f64]) -> Vec<(usize, f64)> {
        let mut v: Vec<(usize, f64)> = addr.iter().copied().zip(vals.iter().copied()).collect();
        v.sort_by_key(|p| p.0);
        v
    }

    /// V&V (verification, 2026-11-27). Identical patches give identity
    /// addressing with weights 1.
    ///
    /// Methodology: (a) a non-uniform 4x3 planar quad grid against itself with
    /// the faces reversed (the two halves of a coincident seam face each other,
    /// the `reverse_target = false` convention); (b) the same grid with equal
    /// winding and `reverse_target = true`; (c) a 16x2 cylinder band against
    /// itself reversed (non-planar). All four combinations of triangulation
    /// (`Mesh`/`Fan`) and `require_match` (conformal / area-fraction
    /// normalisation). Pass: every source face `i` addresses exactly target
    /// `[i]` and vice versa, weights and weight sums within 1e-12 of 1.
    /// (d) Equal winding with `reverse_target = false` is the wrong
    /// convention: the resultant normal `-n_src + n_tgt` vanishes, upstream
    /// warns and takes the overlap as zero; pass: no addressing and an
    /// `InvalidNormal` warning.
    ///
    /// Result (measured 2026-11-27, release build): (a)-(c) identity in all
    /// 12 runs; the largest |w - 1| or |Σw - 1| over all of them is 4.4e-16.
    /// (d) all addresses empty, `InvalidNormal` warnings raised. PASS.
    #[test]
    fn vv_identical_patches_identity() {
        let xs = [0.0, 0.2, 0.45, 0.8, 1.0];
        let ys = [0.0, 0.3, 0.55, 1.0];
        let a = grid_patch(&xs, &ys, false);
        let a_rev = grid_patch(&xs, &ys, true);
        let cyl = cylinder_band(16, 2, 0.4, 0.25, 0.1, false);
        let cyl_in = cylinder_band(16, 2, 0.4, 0.25, 0.1, true);
        let cases = [(&a, &a_rev, false), (&a, &a, true), (&cyl, &cyl_in, false)];
        let mut worst: f64 = 0.0;
        for (src, tgt, reverse_target) in cases {
            for tri in [AmiTriangulationMode::Mesh, AmiTriangulationMode::Fan] {
                for require_match in [true, false] {
                    let opts = FaceAreaWeightAmiOptions {
                        require_match,
                        reverse_target,
                        triangulation: tri,
                        ..Default::default()
                    };
                    let r = face_area_weight_ami(src, tgt, &opts).unwrap();
                    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
                    for i in 0..src.faces.len() {
                        assert_eq!(r.src_address[i], vec![i]);
                        assert_eq!(r.tgt_address[i], vec![i]);
                        for v in [
                            r.src_weights[i][0],
                            r.tgt_weights[i][0],
                            r.src_weights_sum[i],
                            r.tgt_weights_sum[i],
                        ] {
                            worst = worst.max((v - 1.0).abs());
                        }
                    }
                }
            }
        }
        std::println!("identity: worst |w-1| = {worst:e}");
        assert!(worst < 1e-12, "worst {worst:e}");

        // (d) wrong orientation convention
        let r = face_area_weight_ami(&a, &a, &FaceAreaWeightAmiOptions::default()).unwrap();
        assert!(r.src_address.iter().all(|x| x.is_empty()));
        assert!(r
            .warnings
            .iter()
            .any(|w| matches!(w, AmiWarning::InvalidNormal { .. })));
    }

    /// V&V (verification, 2026-11-27). On a structured planar seam the 3-D
    /// clipping reproduces the 1-D interval overlap of [`overlap_weights_1d`].
    ///
    /// Methodology: strips along x of depth 0.7 in y. Case A: the 2:1 split,
    /// target spans `[0,0.5],[0.5,1]` over source `[0,0.25],…,[0.75,1]`;
    /// case B: target `[0,0.3],[0.3,1]` over the same sources (a partial
    /// overlap). `require_match = false`, so `tgt_weights` are overlap /
    /// target area, the definition `overlap_weights_1d` uses. Compared per
    /// target face (sorted by source index): the source set, the weight and
    /// the raw overlap area; and the same from the source side against
    /// `overlap_weights_1d(source, target)`. Pass: identical sets, values
    /// within 1e-12.
    ///
    /// Result (measured 2026-11-27, release build): identical sets in both
    /// cases and from both sides; largest difference over weights and areas
    /// 2.2e-16. PASS.
    #[test]
    fn vv_planar_split_matches_overlap_weights_1d() {
        let depth = 0.7;
        let ys = [0.0, depth];
        let src_xs = [0.0, 0.25, 0.5, 0.75, 1.0];
        let mut worst: f64 = 0.0;
        for tgt_xs in [vec![0.0, 0.5, 1.0], vec![0.0, 0.3, 1.0]] {
            let src = grid_patch(&src_xs, &ys, false);
            let tgt = grid_patch(&tgt_xs, &ys, true);
            let opts = FaceAreaWeightAmiOptions {
                require_match: false,
                ..Default::default()
            };
            let r = face_area_weight_ami(&src, &tgt, &opts).unwrap();
            let spans =
                |xs: &[f64]| -> Vec<(f64, f64)> { xs.windows(2).map(|w| (w[0], w[1])).collect() };
            let one_d_t = overlap_weights_1d(&spans(&tgt_xs), &spans(&src_xs), depth);
            let one_d_s = overlap_weights_1d(&spans(&src_xs), &spans(&tgt_xs), depth);
            for (addr, wts, areas, one_d) in [
                (
                    &r.tgt_address,
                    &r.tgt_weights,
                    &r.tgt_overlap_areas,
                    &one_d_t,
                ),
                (
                    &r.src_address,
                    &r.src_weights,
                    &r.src_overlap_areas,
                    &one_d_s,
                ),
            ] {
                for (i, row) in one_d.iter().enumerate() {
                    let w = sorted_pairs(&addr[i], &wts[i]);
                    let a = sorted_pairs(&addr[i], &areas[i]);
                    assert_eq!(w.len(), row.len(), "face {i}: {w:?} vs {row:?}");
                    for (k, o) in row.iter().enumerate() {
                        assert_eq!(w[k].0, o.source);
                        worst = worst.max((w[k].1 - o.weight).abs());
                        worst = worst.max((a[k].1 - o.overlap_area).abs());
                    }
                }
            }
        }
        std::println!("1-D agreement: worst = {worst:e}");
        assert!(worst < 1e-12, "worst {worst:e}");
    }

    /// The unstructured planar seam of the coverage tests: a perturbed 5x4
    /// source grid against a perturbed 3x3 target grid whose first row's
    /// first two quads are merged into one hexagon (two collinear vertices),
    /// both over `[0, 1.3] x [0, 0.9]`.
    fn unstructured_planar_seam() -> (AmiPatchGeometry, AmiPatchGeometry) {
        let sx = linspace(0.0, 1.3, 5);
        let sy = linspace(0.0, 0.9, 4);
        let mut src = grid_patch(&sx, &sy, false);
        perturb_interior(&mut src, &sx, &sy, 0.4, 7);
        let tx = linspace(0.0, 1.3, 3);
        let ty = linspace(0.0, 0.9, 3);
        let mut tgt = grid_patch(&tx, &ty, true);
        perturb_interior(&mut tgt, &tx, &ty, 0.4, 11);
        // Merge target faces 0 and 1 (row 0) into a hexagon, keeping -z
        // winding: the reversed outline of quads (0,0)-(1,0) and (1,0)-(2,0).
        let nx = tx.len();
        let v = |a: usize, b: usize| b * nx + a;
        let hex = vec![v(0, 0), v(0, 1), v(1, 1), v(2, 1), v(2, 0), v(1, 0)];
        tgt.faces[0] = hex;
        tgt.faces.remove(1);
        (src, tgt)
    }

    /// V&V (verification, 2026-11-27). On a fully covered seam every face's
    /// weight sum is 1 from both sides, and the total intersected area agrees
    /// from both sides and with the seam area.
    ///
    /// Methodology: [`unstructured_planar_seam`] (non-conformal, skewed quads,
    /// a hexagon with collinear vertices), in all four combinations of
    /// triangulation (`Mesh`/`Fan`) and area normalisation (`Project`/`Mag`),
    /// `require_match = true`. Pass: every `src_weights_sum` and
    /// `tgt_weights_sum` within 1e-10 of 1 (the seam is planar and the
    /// patches share their outline, so the covered fraction is 1 up to
    /// round-off and the 1e-6 coplanarity snap); every face's conformal
    /// weights sum to 1 within 1e-14; Σ raw overlap areas equal from both
    /// sides within 1e-14 relative, and equal to the seam area 1.17 m² within
    /// 1e-12 relative.
    ///
    /// Result (measured 2026-11-27, release build): worst |Σw - 1| 6.7e-16
    /// (`Mesh`) and 8.9e-16 (`Fan`), identical for `Project` and `Mag`;
    /// worst conformal-sum error 2.2e-16; Σ raw areas src vs tgt 1.9e-16
    /// relative; vs the seam area 5.7e-16 relative. PASS.
    #[test]
    fn vv_fully_covered_seam_weight_sums_and_total_area() {
        let (src, tgt) = unstructured_planar_seam();
        let seam_area = 1.3 * 0.9;
        for tri in [AmiTriangulationMode::Mesh, AmiTriangulationMode::Fan] {
            for norm in [AmiAreaNormalisation::Project, AmiAreaNormalisation::Mag] {
                let opts = FaceAreaWeightAmiOptions {
                    triangulation: tri,
                    area_normalisation: norm,
                    ..Default::default()
                };
                let r = face_area_weight_ami(&src, &tgt, &opts).unwrap();
                assert!(r.warnings.is_empty(), "{:?}", r.warnings);
                let mut worst_sum: f64 = 0.0;
                let mut worst_conf: f64 = 0.0;
                for (sums, wts) in [
                    (&r.src_weights_sum, &r.src_weights),
                    (&r.tgt_weights_sum, &r.tgt_weights),
                ] {
                    for (s, w) in sums.iter().zip(wts.iter()) {
                        worst_sum = worst_sum.max((s - 1.0).abs());
                        let c: f64 = w.iter().sum();
                        worst_conf = worst_conf.max((c - 1.0).abs());
                    }
                }
                let tot_src: f64 = r.src_overlap_areas.iter().flatten().sum();
                let tot_tgt: f64 = r.tgt_overlap_areas.iter().flatten().sum();
                let both = ((tot_src - tot_tgt) / tot_src).abs();
                let vs_seam = ((tot_src - seam_area) / seam_area).abs();
                std::println!(
                    "{tri:?}/{norm:?}: worst |sum-1| = {worst_sum:e}, conformal {worst_conf:e}, \
                     src/tgt total rel diff {both:e}, vs seam {vs_seam:e}"
                );
                assert!(worst_sum < 1e-10, "{worst_sum:e}");
                assert!(worst_conf < 1e-14, "{worst_conf:e}");
                assert!(both < 1e-14, "{both:e}");
                assert!(vs_seam < 1e-12, "{vs_seam:e}");
            }
        }
    }

    /// Wraps an angle into `(-pi, pi]`.
    fn wrap(a: f64) -> f64 {
        let mut x = a;
        while x > PI {
            x -= 2.0 * PI;
        }
        while x <= -PI {
            x += 2.0 * PI;
        }
        x
    }

    /// Semi-analytic projection model of the cylindrical seam (see
    /// [`vv_cylindrical_seam_weight_sums_match_facet_model`]): width of the
    /// projection of target column `jt` on source column `js`, in the source
    /// facet's tangential coordinate.
    fn cylinder_projected_width(
        n1: usize,
        th1: f64,
        js: usize,
        n2: usize,
        th2: f64,
        jt: usize,
        r: f64,
    ) -> f64 {
        let beta = PI / n1 as f64;
        let d1 = r * beta.r_cos();
        let half_c1 = r * beta.r_sin();
        let theta_s = th1 + 2.0 * PI * (js as f64 + 0.5) / n1 as f64;
        let theta_t = th2 + 2.0 * PI * (jt as f64 + 0.5) / n2 as f64;
        let delta = wrap(theta_t - theta_s);
        if delta.abs() > 0.5 * PI {
            return 0.0;
        }
        // Projection direction: the resultant normal -(n_s + n_t), the
        // bisector of the two facet normals, at angle delta/2 from n_s.
        let tan_phi = (0.5 * delta).r_tan();
        let y = |alpha: f64| r * alpha.r_sin() + (d1 - r * alpha.r_cos()) * tan_phi;
        let y_l = y(delta - PI / n2 as f64);
        let y_r = y(delta + PI / n2 as f64);
        (y_r.min(half_c1) - y_l.max(-half_c1)).max(0.0)
    }

    /// V&V (verification, 2026-11-27). A cylindrical seam: two tessellations
    /// of the same band with different facet counts and axial divisions.
    ///
    /// Methodology: radius 0.5 m, height 0.3 m; source 24 facets x 3 rows
    /// (outward normals, first vertex at 0 rad), target 36 facets x 5 rows
    /// (inward, first vertex at 0.013 rad so no edges coincide); defaults
    /// (`require_match`, `tmMesh`, `project`).
    ///
    /// **Prediction, written before the first run.** Both tessellations are
    /// inscribed, so their facets are different chords and do not coincide.
    /// Upstream clips a source triangle by planes through each target edge
    /// *containing the pair's resultant normal* `-(n_s + n_t)`, i.e. it
    /// projects the target facet onto the source facet along the **bisector**
    /// of the two facet normals. Horizontal target edges project to exact
    /// `z = const` cuts, so the different axial divisions cost nothing. An
    /// axial target edge at angle `alpha` (relative to the source facet
    /// centre) lands at tangential coordinate
    /// `y = R sin(alpha) + (R cos(beta) - R cos(alpha)) tan(phi)`,
    /// `beta = pi/N1`, `phi` = half the angle between the facet centres. The
    /// two target facets sharing that edge use different `phi`, so inside a
    /// source facet (where `R cos(beta) < R cos(alpha)`) their projections
    /// **overlap**: the source side is over-covered, not under-covered. To
    /// leading order the mean excess is `pi^2/(6 N1^2)` = **2.86e-3** for the
    /// source weight sums (`N1 = 24`), individual faces between 1 and about
    /// `1 + 2 pi^2/(4 N1 N2)` ≈ 1.0057, and the target sums average
    /// `1 + pi^2/(6 N2^2)` = **1.27e-3** above 1 (`N2 = 36`; the facet-area
    /// ratio `N1 sin(pi/N1) / (N2 sin(pi/N2))` and the source excess
    /// combined). The exact prediction per face is the 1-D union of the
    /// projected intervals ([`cylinder_projected_width`]); the code should
    /// match it to round-off plus the 1e-6 coplanarity snap. Pass: every
    /// weight sum within 1e-9 of that model; mean excesses within 20 % of the
    /// leading-order numbers; Σ raw areas equal from both sides within 1e-14
    /// relative.
    ///
    /// Result (measured 2026-11-27, release build): source sums in
    /// [1.0027054, 1.0028687], mean excess **2.787e-3** (leading-order
    /// prediction 2.856e-3, 2.4 % high); target mean excess **1.196e-3**
    /// (prediction 1.269e-3, 6.1 % high); worst disagreement with the exact
    /// facet model 2.7e-15 (source) and 4.9e-15 (target); Σ raw areas equal
    /// from both sides (0 relative). The sign and mean size held: the
    /// departure from 1 is an **excess**, caused by the bisector projection
    /// of mismatched chords. The predicted per-face spread was wrong: the
    /// faces lie within 1.6e-4 of each other, not between 1 and 1.0057 (the
    /// overlaps of the 1-2 target edges inside each source facet nearly sum
    /// to a constant). PASS.
    #[test]
    fn vv_cylindrical_seam_weight_sums_match_facet_model() {
        let (r, h) = (0.5, 0.3);
        let (n1, m1, th1) = (24, 3, 0.0);
        let (n2, m2, th2) = (36, 5, 0.013);
        let src = cylinder_band(n1, m1, r, h, th1, false);
        let tgt = cylinder_band(n2, m2, r, h, th2, true);
        let res = face_area_weight_ami(&src, &tgt, &FaceAreaWeightAmiOptions::default()).unwrap();
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);

        let c1 = 2.0 * r * (PI / n1 as f64).r_sin();
        let c2 = 2.0 * r * (PI / n2 as f64).r_sin();
        let mut worst_src: f64 = 0.0;
        let mut worst_tgt: f64 = 0.0;
        let (mut mn, mut mx, mut mean_s) = (f64::MAX, f64::MIN, 0.0);
        for s in 0..n1 * m1 {
            let js = s % n1;
            let model: f64 = (0..n2)
                .map(|jt| cylinder_projected_width(n1, th1, js, n2, th2, jt, r))
                .sum::<f64>()
                / c1;
            let got = res.src_weights_sum[s];
            worst_src = worst_src.max((got - model).abs());
            mn = mn.min(got);
            mx = mx.max(got);
            mean_s += got / (n1 * m1) as f64;
        }
        let mut mean_t = 0.0;
        for t in 0..n2 * m2 {
            let jt = t % n2;
            let model: f64 = (0..n1)
                .map(|js| cylinder_projected_width(n1, th1, js, n2, th2, jt, r))
                .sum::<f64>()
                / c2;
            let got = res.tgt_weights_sum[t];
            worst_tgt = worst_tgt.max((got - model).abs());
            mean_t += got / (n2 * m2) as f64;
        }
        let tot_src: f64 = res.src_overlap_areas.iter().flatten().sum();
        let tot_tgt: f64 = res.tgt_overlap_areas.iter().flatten().sum();
        let both = ((tot_src - tot_tgt) / tot_src).abs();
        let pred_s = PI * PI / (6.0 * (n1 * n1) as f64);
        let pred_t = PI * PI / (6.0 * (n2 * n2) as f64);
        std::println!(
            "cylinder: src sums [{mn:.7}, {mx:.7}] mean excess {:.4e} (pred {pred_s:.4e}); \
             tgt mean excess {:.4e} (pred {pred_t:.4e}); model worst src {worst_src:e} tgt {worst_tgt:e}; \
             totals rel diff {both:e}",
            mean_s - 1.0,
            mean_t - 1.0
        );
        assert!(worst_src < 1e-9, "{worst_src:e}");
        assert!(worst_tgt < 1e-9, "{worst_tgt:e}");
        assert!(((mean_s - 1.0) / pred_s - 1.0).abs() < 0.2);
        assert!(((mean_t - 1.0) / pred_t - 1.0).abs() < 0.2);
        assert!(mn > 1.0 - 1e-12);
        assert!(both < 1e-14, "{both:e}");
    }

    /// V&V (verification, 2026-11-27). The stalled-walk and restart paths.
    ///
    /// Methodology: the walk always starts from source face 0 against target
    /// face 0 (upstream `initialiseWalk(0, 0)` does not search). Listing the
    /// target faces of [`unstructured_planar_seam`] in reverse puts target 0
    /// in the far corner, so source 0 finds no overlap, is recorded in
    /// `src_non_overlap`, and the walk continues through `setNextFaces`'
    /// nearest-face search. Pass: (a) with the restart (default) the result
    /// equals the forward-ordered result face by face (same overlap sets,
    /// areas within 1e-14) and `src_non_overlap == [0]`; (b) without the
    /// restart, source 0 stays unaddressed with weight sum 0 (upstream never
    /// revisits a source face once the walk has passed it).
    ///
    /// Result (measured 2026-11-27, release build): (a) identical sets,
    /// worst area difference 0 (bit-identical); (b) as stated. PASS.
    #[test]
    fn vv_stalled_walk_and_restart() {
        let (src, tgt) = unstructured_planar_seam();
        let n_t = tgt.faces.len();
        let mut tgt_rev = tgt.clone();
        tgt_rev.faces.reverse();
        let fwd = face_area_weight_ami(&src, &tgt, &FaceAreaWeightAmiOptions::default()).unwrap();
        let rev =
            face_area_weight_ami(&src, &tgt_rev, &FaceAreaWeightAmiOptions::default()).unwrap();
        assert!(fwd.src_non_overlap.is_empty());
        assert_eq!(rev.src_non_overlap, vec![0]);
        let mut worst: f64 = 0.0;
        for s in 0..src.faces.len() {
            let a = sorted_pairs(&fwd.src_address[s], &fwd.src_overlap_areas[s]);
            let mapped: Vec<usize> = rev.src_address[s].iter().map(|&t| n_t - 1 - t).collect();
            let b = sorted_pairs(&mapped, &rev.src_overlap_areas[s]);
            assert_eq!(a.len(), b.len(), "source {s}");
            for (x, y) in a.iter().zip(b.iter()) {
                assert_eq!(x.0, y.0);
                worst = worst.max((x.1 - y.1).abs());
            }
        }
        std::println!("stalled walk: worst area difference vs forward order = {worst:e}");
        assert!(worst < 1e-14, "{worst:e}");
        let no_restart = FaceAreaWeightAmiOptions {
            restart_uncovered_source_face: false,
            ..Default::default()
        };
        let r = face_area_weight_ami(&src, &tgt_rev, &no_restart).unwrap();
        assert!(r.src_address[0].is_empty());
        assert_eq!(r.src_weights_sum[0], 0.0);
        assert!(r.src_address[1..].iter().all(|a| !a.is_empty()));
    }

    /// `to_ami_couplings` reproduces the couplings `FvMesh::periodic_ring_ami`
    /// builds from `overlap_weights_1d` for its mid seam, and they build a mesh.
    ///
    /// Methodology: the ring's mid seam (`n_a = 2` target faces `A_right` at
    /// x = 0.5 m, `n_b = 4` source faces `B_left`, depth 1 m) as 3-D geometry,
    /// through `face_area_weight_ami` (defaults) and
    /// `to_ami_couplings(Target, ..)`; compared field by field with the ring's
    /// first two couplings. Pass: same faces, cells, patches; weights and
    /// overlap areas within 1e-14; the mesh builds with them.
    /// Result (measured 2026-11-27, release build): equal; worst difference
    /// 1.1e-16; the mesh builds. PASS.
    #[test]
    fn vv_to_ami_couplings_matches_periodic_ring() {
        let (n_a, n_b) = (2usize, 4usize);
        let ring = FvMesh::periodic_ring_ami(n_a, n_b, 1.0, 1.0, 1.0);
        let strip = |n: usize, plus_x: bool| {
            let mut points = Vec::new();
            for i in 0..=n {
                let y = i as f64 / n as f64;
                points.push(Vector3::new(0.5, y, 0.0));
                points.push(Vector3::new(0.5, y, 1.0));
            }
            let faces = (0..n)
                .map(|i| {
                    let f = vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1];
                    if plus_x {
                        f
                    } else {
                        f.into_iter().rev().collect()
                    }
                })
                .collect();
            AmiPatchGeometry { points, faces }
        };
        let tgt = strip(n_a, true); // A_right, +x
        let src = strip(n_b, false); // B_left, -x
        let res = face_area_weight_ami(&src, &tgt, &FaceAreaWeightAmiOptions::default()).unwrap();
        let coupled = AmiPatchPlacement {
            patch: 0,
            start: 0,
            face_cells: (0..n_a).collect(),
        };
        let partner = AmiPatchPlacement {
            patch: 1,
            start: n_a,
            face_cells: (n_a..n_a + n_b).collect(),
        };
        let cc = res.to_ami_couplings(AmiPatchSide::Target, &coupled, &partner);
        assert_eq!(cc.len(), n_a);
        let mut worst: f64 = 0.0;
        for (mine, theirs) in cc.iter().zip(ring.ami_couplings.iter()) {
            assert_eq!(mine.target_face, theirs.target_face);
            assert_eq!(mine.target_cell, theirs.target_cell);
            assert_eq!(mine.target_patch, theirs.target_patch);
            assert_eq!(mine.source_patch, theirs.source_patch);
            assert_eq!(mine.local, theirs.local);
            let mut w = mine.weights.clone();
            w.sort_by_key(|x| x.source_face);
            assert_eq!(w.len(), theirs.weights.len());
            for (a, b) in w.iter().zip(theirs.weights.iter()) {
                assert_eq!(a.source_face, b.source_face);
                assert_eq!(a.source_cell, b.source_cell);
                worst = worst.max((a.weight - b.weight).abs());
                worst = worst.max((a.overlap_area - b.overlap_area).abs());
            }
        }
        std::println!("to_ami_couplings vs periodic_ring_ami: worst = {worst:e}");
        assert!(worst < 1e-14, "{worst:e}");
        // The source side gives one coupling per B face.
        let back = res.to_ami_couplings(AmiPatchSide::Source, &partner, &coupled);
        assert_eq!(back.len(), n_b);
        assert!(back.iter().all(|c| (c.weight_sum() - 1.0).abs() < 1e-14));
        // And the couplings build a mesh in place of the ring's own.
        let mut all = cc;
        all.extend(ring.ami_couplings[n_a..].iter().cloned());
        let rebuilt = crate::mesh::FvMeshBuilder::new()
            .n_cells(ring.n_cells)
            .n_internal_faces(0)
            .owner(ring.owner.clone())
            .neighbour(vec![])
            .patches(ring.patches.clone())
            .ami_couplings(all)
            .cell_volumes(ring.cell_volumes.clone())
            .cell_centres(ring.cell_centres.clone())
            .face_area_vectors(ring.face_area_vectors.clone())
            .face_centres(ring.face_centres.clone())
            .build();
        assert!(rebuilt.is_ok(), "{:?}", rebuilt.err());
    }

    /// Upstream's fatal conditions and `mustMatchFaces()`.
    ///
    /// A source patch with a second face 1 m off the seam, `max_distance2 =
    /// 0.01 m²`: no target face is a candidate for it, so `setNextFaces`
    /// fails. With `require_match` (and no `lowWeightCorrection`) that is
    /// `UnableToSetTargetFace`; with `low_weight_correction = 0.2` the walk
    /// just ends, the face stays empty (and, as upstream, is not counted in the low-weight
    /// statistic, which only looks at faces that have weights). The source
    /// bounding box then also overshoots the target's (warning). Malformed
    /// faces give `PointIndexOutOfRange` and, under `tmMesh`,
    /// `FaceTooFewVertices`; an empty source patch gives empty results.
    #[test]
    fn errors_and_must_match_faces() {
        let tgt = grid_patch(&[0.0, 1.0], &[0.0, 1.0], true);
        let mut src = grid_patch(&[0.0, 1.0], &[0.0, 1.0], false);
        let base = src.points.len();
        for p in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            src.points.push(Vector3::new(p.0 + 5.0, p.1, 1.0));
        }
        src.faces.push(vec![base, base + 1, base + 2, base + 3]);
        let strict = FaceAreaWeightAmiOptions {
            max_distance2: 0.01,
            ..Default::default()
        };
        assert_eq!(
            face_area_weight_ami(&src, &tgt, &strict),
            Err(AmiError::UnableToSetTargetFace { source_face: 1 })
        );
        let lenient = FaceAreaWeightAmiOptions {
            low_weight_correction: 0.2,
            ..strict
        };
        let r = face_area_weight_ami(&src, &tgt, &lenient).unwrap();
        assert_eq!(r.src_address[0], vec![0]);
        assert!(r.src_address[1].is_empty());
        // Upstream counts a low-weight face only among faces WITH weights.
        assert_eq!(r.src_n_low_weight, 0);
        assert!(r.warnings.contains(&AmiWarning::BoundingBoxesNotSimilar));

        let mut bad = tgt.clone();
        bad.faces[0][0] = 99;
        assert!(matches!(
            face_area_weight_ami(&src, &bad, &lenient),
            Err(AmiError::PointIndexOutOfRange {
                side: AmiPatchSide::Target,
                face: 0,
                point: 99
            })
        ));
        let mut two = tgt.clone();
        two.faces[0] = vec![0, 1];
        assert!(matches!(
            face_area_weight_ami(&src, &two, &lenient),
            Err(AmiError::FaceTooFewVertices {
                side: AmiPatchSide::Target,
                face: 0
            })
        ));
        let msg = std::format!("{}", AmiError::UnableToSetTargetFace { source_face: 3 });
        assert!(msg.contains("source face 3"));

        let empty = AmiPatchGeometry {
            points: vec![],
            faces: vec![],
        };
        let r = face_area_weight_ami(&empty, &tgt, &lenient).unwrap();
        assert!(r.src_address.is_empty() && r.tgt_address.len() == 1);
        let r = face_area_weight_ami(&src, &empty, &lenient).unwrap();
        assert!(r.warnings.contains(&AmiWarning::NoTargetFaces));
        assert!(r.src_address.iter().all(|a| a.is_empty()));
    }
}
