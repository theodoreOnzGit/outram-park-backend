// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithm structure derived from OpenFOAM (www.openfoam.com)
//   `applications/utilities/mesh/generation/blockMesh` and
//   `src/mesh/blockMesh` (blockDescriptor / block point generation), and the
//   `hex` cellModel face ordering + primitiveMesh face/cell geometry algorithms
//   (`src/OpenFOAM/meshes/primitiveMesh/primitiveMeshFaceCentresAndAreas.C`,
//   `primitiveMeshCellCentresAndVols.C`).
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! `blockMesh` — structured hexahedral block meshing from a `blockMeshDict`.
//!
//! Reads a `blockMeshDict` (vertices, blocks with cell counts + grading, edges,
//! boundary patches) and produces a `polyMesh` (points, faces, owner/neighbour,
//! boundary patches) via the standard OpenFOAM `blockMesh` algorithm: each block
//! is a hexahedron mapped from the unit cube through its 8 vertices, subdivided
//! by its `(nx, ny, nz)` cell counts with optional `simpleGrading` geometric
//! expansion; faces that coincide on shared block boundaries are merged into
//! internal faces.
//!
//! ## Pipeline
//!
//! 1. [`BlockMeshDict::parse`] tokenises and parses the dict text into a
//!    [`BlockMeshDict`] (`convertToMeters`, [`vertices`](BlockMeshDict::vertices),
//!    [`blocks`](BlockMeshDict::blocks), boundary [`patches`](BlockMeshDict::patches)).
//! 2. [`BlockMeshDict::build`] subdivides every block, merges globally coincident
//!    points, deduplicates faces (a face shared by two cells becomes internal),
//!    assigns the remaining boundary faces to patches, and returns a [`PolyMesh`].
//! 3. [`PolyMesh::to_fv_mesh`] computes the finite-volume geometry (cell volumes
//!    and centres, face-area vectors and centres) and emits the
//!    `outram-foam-basic-lib` [`FvMesh`].
//!
//! The convenience free function [`block_mesh`] runs the whole pipeline.
//!
//! ## Units
//!
//! Dict coordinates are dimensionless and are multiplied by `convertToMeters`
//! (metres per dict unit) to obtain SI positions. All emitted geometry is SI:
//! points/centres in metres `[m]`, face areas in `[m^2]`, cell volumes in
//! `[m^3]`.
//!
//! ## Deferred dict features
//!
//! - `edges` blocks (arc / spline / polyLine) are parsed and **skipped**: all
//!   block edges are treated as straight lines (bilinear/trilinear block map).
//!   Curved-edge point projection is a later phase.
//! - `simpleGrading (gx gy gz)` is fully supported, where each of the three
//!   directions is **either** a single scalar expansion ratio **or** a
//!   multi-grading list of `( fractionLength fractionCells expansionRatio )`
//!   segments (OpenFOAM `gradingDescriptors`), graded piecewise-geometrically.
//! - `edgeGrading (e0 … e11)` is supported **only** in the restricted case
//!   where the four edges of each local direction share the same grading (then
//!   it is exact, equivalent to `simpleGrading`). A genuinely per-edge
//!   (non-planar) `edgeGrading` returns [`MeshError::NotImplemented`], because
//!   this crate uses one node distribution per direction rather than OpenFOAM's
//!   full 12-edge blend.
//! - `mergePatchPairs` (face-merging of separately-meshed patch pairs) is
//!   parsed and ignored; coincident-point merging across blocks is always done.

use std::collections::HashMap;
use std::path::Path;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::MeshError;

// ───────────────────────────────────────────────────────────────────────────
//  Hex topology constants (OpenFOAM `hex` cellModel)
// ───────────────────────────────────────────────────────────────────────────

/// Unit-cube coordinates of the 8 hex vertices, in OpenFOAM local ordering.
///
/// `v0=(0,0,0) v1=(1,0,0) v2=(1,1,0) v3=(0,1,0) v4=(0,0,1) v5=(1,0,1)
///  v6=(1,1,1) v7=(0,1,1)`.
const HEX_CORNER: [[f64; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
];

/// The 6 faces of the `hex` cellModel, as local-vertex quads wound so the
/// normal points **out** of the cell. Order: x-min, x-max, y-min, y-max,
/// z-min, z-max.
const HEX_FACE: [[usize; 4]; 6] = [
    [0, 4, 7, 3], // x-min
    [1, 2, 6, 5], // x-max
    [0, 1, 5, 4], // y-min
    [3, 7, 6, 2], // y-max
    [0, 3, 2, 1], // z-min
    [4, 5, 6, 7], // z-max
];

// ───────────────────────────────────────────────────────────────────────────
//  Parsed dictionary types
// ───────────────────────────────────────────────────────────────────────────

/// One segment of a multi-grading list, as it appears inside a parenthesised
/// per-direction grading entry `( fractionLength fractionCells expansionRatio )`.
///
/// Mirrors OpenFOAM's `Foam::gradingDescriptor`
/// (`src/mesh/blockMesh/gradingDescriptor/gradingDescriptor.C`). All three
/// fields are **dimensionless**:
///
/// - `fraction_length` — the fraction of the local block edge length this
///   segment occupies (OpenFOAM `blockFraction`). Relative; the segment
///   fractions of a direction are normalised to sum to `1` at mesh-build time.
/// - `fraction_cells` — the fraction of the direction's cell count allocated to
///   this segment (OpenFOAM `nDivFraction`). Relative; likewise normalised.
/// - `expansion` — the geometric expansion ratio over the segment, defined as
///   `end-cell-width / start-cell-width`. `1.0` is uniform spacing; a negative
///   value is trapped and treated as its inverse (OpenFOAM
///   `gradingDescriptor::correct`).
#[derive(Debug, Clone, PartialEq)]
pub struct GradingSegment {
    /// Fraction of the block edge length occupied by this segment
    /// (dimensionless, relative; normalised to sum to `1` per direction).
    pub fraction_length: f64,
    /// Fraction of the direction's cell count in this segment (dimensionless,
    /// relative; normalised to sum to `1` per direction).
    pub fraction_cells: f64,
    /// Geometric expansion ratio `end-cell-width / start-cell-width`
    /// (dimensionless, `> 0`; negatives trapped as their inverse).
    pub expansion: f64,
}

/// The node distribution along one local block direction.
///
/// Mirrors OpenFOAM's `Foam::gradingDescriptors`
/// (`src/mesh/blockMesh/gradingDescriptor/gradingDescriptors.C`): a direction is
/// either a single `simpleGrading` expansion ratio, or a list of
/// [`GradingSegment`]s (multi-grading). Both are dimensionless — they only
/// redistribute the `n + 1` node positions along the `[0, 1]` parametric edge,
/// never the cell count.
#[derive(Debug, Clone, PartialEq)]
pub enum Grading {
    /// A single expansion ratio (OpenFOAM scalar `simpleGrading` entry),
    /// `end-cell-width / start-cell-width`; `1.0` is uniform.
    Uniform(f64),
    /// A multi-grading list: the direction is split into contiguous
    /// [`GradingSegment`]s, each graded geometrically over its own portion of
    /// the edge. Segments are listed start → end along the local direction.
    Multi(Vec<GradingSegment>),
}

/// Equality against a bare expansion ratio, so a `[Grading; 3]` compares equal
/// to a `[f64; 3]` of expansion ratios (a [`Grading::Uniform`] equals its
/// scalar ratio; a [`Grading::Multi`] never equals a scalar). This keeps the
/// scalar `simpleGrading` case ergonomic for callers and tests that think in
/// terms of the three plain ratios.
impl PartialEq<f64> for Grading {
    fn eq(&self, other: &f64) -> bool {
        match self {
            Grading::Uniform(r) => r == other,
            Grading::Multi(_) => false,
        }
    }
}

impl Grading {
    /// Normalised node positions `[0, 1]` for `n` cells under this grading.
    ///
    /// Returns `n + 1` monotonically increasing positions with `s[0] == 0` and
    /// `s[n] == 1`. For [`Grading::Uniform`] this is the geometric single-ratio
    /// distribution; for [`Grading::Multi`] it is the piecewise-geometric
    /// multi-segment distribution (see [`graded_positions_multi`]).
    fn node_positions(&self, n: usize) -> Vec<f64> {
        match self {
            Grading::Uniform(r) => graded_positions(n, *r),
            Grading::Multi(segments) => graded_positions_multi(n, segments),
        }
    }
}

/// One `hex` block from the `blocks` list.
///
/// The block is a hexahedron whose 8 corners are indices into
/// [`BlockMeshDict::vertices`], subdivided into `cells[0] * cells[1] * cells[2]`
/// hexahedral cells. `grading` holds the three per-direction [`Grading`]
/// distributions (from `simpleGrading` or, when all four edges of a direction
/// agree, `edgeGrading`); each direction only redistributes node positions
/// along its `[0, 1]` parametric edge (`1.0` uniform expansion is even
/// spacing).
#[derive(Debug, Clone)]
pub struct Block {
    /// The 8 block-corner vertex indices, in OpenFOAM hex order.
    pub vertices: [usize; 8],
    /// Cell counts `(nx, ny, nz)` along the three local directions.
    pub cells: [usize; 3],
    /// Per-direction node distributions `(gx, gy, gz)`.
    pub grading: [Grading; 3],
}

/// One named boundary patch from the `boundary` list.
///
/// `faces` are quads given as **block-corner** vertex indices (into
/// [`BlockMeshDict::vertices`]) — the coarse block-face they cover, not the fine
/// mesh faces. `build` expands each coarse quad to all the fine boundary faces
/// lying on that block face.
#[derive(Debug, Clone)]
pub struct PatchDef {
    /// Patch name (e.g. `"movingWall"`).
    pub name: String,
    /// Topological patch kind, mapped from the dict `type` keyword.
    pub kind: PatchKind,
    /// Coarse block-face quads (block-corner vertex indices).
    pub faces: Vec<[usize; 4]>,
}

/// A parsed `blockMeshDict`.
///
/// Produced by [`BlockMeshDict::parse`]; consumed by [`BlockMeshDict::build`].
#[derive(Debug, Clone)]
pub struct BlockMeshDict {
    /// Scale factor `[m per dict unit]` applied to every vertex (`convertToMeters`).
    pub convert_to_meters: f64,
    /// The vertex list (raw, unscaled dict coordinates).
    pub vertices: Vec<Vector3>,
    /// The `hex` blocks.
    pub blocks: Vec<Block>,
    /// The named boundary patches.
    pub patches: Vec<PatchDef>,
}

// ───────────────────────────────────────────────────────────────────────────
//  Output poly-mesh types
// ───────────────────────────────────────────────────────────────────────────

/// A single mesh face: its point-index loop plus owner / neighbour cells.
///
/// `verts` is wound so the face normal points **from `owner` towards
/// `neighbour`** (outward from the owner cell). Boundary faces have
/// `neighbour == None` and their normal points out of the domain.
#[derive(Debug, Clone)]
pub struct MeshFace {
    /// Ordered point indices (into [`PolyMesh::points`]) forming the face loop.
    pub verts: Vec<usize>,
    /// Owning cell index.
    pub owner: usize,
    /// Neighbour cell index (internal faces only).
    pub neighbour: Option<usize>,
}

/// The generated poly-mesh: merged points, ordered faces, and boundary patches.
///
/// Faces are ordered OpenFOAM-style: internal faces first
/// (`[0, n_internal_faces)`), then boundary faces grouped by patch in dict
/// order. This is the crate's own lightweight `polyMesh`; call
/// [`PolyMesh::to_fv_mesh`] to obtain the `outram-foam-basic-lib` [`FvMesh`]
/// with full finite-volume geometry.
#[derive(Debug, Clone)]
pub struct PolyMesh {
    /// Mesh points `[m]` (already scaled by `convertToMeters`, coincident block
    /// nodes merged).
    pub points: Vec<Vector3>,
    /// All faces, internal first then boundary (see struct docs).
    pub faces: Vec<MeshFace>,
    /// Number of internal faces (the count of leading internal entries in
    /// `faces`).
    pub n_internal_faces: usize,
    /// Number of cells.
    pub n_cells: usize,
    /// Boundary patches, covering `[n_internal_faces, faces.len())` contiguously.
    pub patches: Vec<BoundaryPatch>,
}

impl PolyMesh {
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.points.len()
    }
    /// Total number of faces (internal + boundary).
    pub fn n_faces(&self) -> usize {
        self.faces.len()
    }
    /// Number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.faces.len() - self.n_internal_faces
    }

    /// Total mesh volume `[m^3]` — the sum of all cell volumes.
    ///
    /// Computed with the same divergence-theorem pyramid decomposition as
    /// [`Self::to_fv_mesh`]; useful as a cheap V&V sanity check against the
    /// analytic domain volume.
    pub fn total_volume(&self) -> f64 {
        self.cell_geometry().0.iter().sum()
    }

    /// Compute per-cell `(volumes, centres)` `[m^3]`, `[m]` via the OpenFOAM
    /// pyramid decomposition (`primitiveMeshCellCentresAndVols`).
    fn cell_geometry(&self) -> (Vec<f64>, Vec<Vector3>) {
        // Face geometry first.
        let mut face_ctr = Vec::with_capacity(self.faces.len());
        let mut face_sf = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (c, sf) = face_centre_and_area(&self.points, &f.verts);
            face_ctr.push(c);
            face_sf.push(sf);
        }

        // First pass: estimate each cell centre as the mean of its face centres.
        let mut c_est = vec![Vector3::ZERO; self.n_cells];
        let mut n_cell_faces = vec![0.0f64; self.n_cells];
        for (fi, f) in self.faces.iter().enumerate() {
            c_est[f.owner] += face_ctr[fi];
            n_cell_faces[f.owner] += 1.0;
            if let Some(nb) = f.neighbour {
                c_est[nb] += face_ctr[fi];
                n_cell_faces[nb] += 1.0;
            }
        }
        for c in 0..self.n_cells {
            if n_cell_faces[c] > 0.0 {
                c_est[c] /= n_cell_faces[c];
            }
        }

        // Second pass: sum pyramid volumes / centroids about the estimate.
        let mut vol = vec![0.0f64; self.n_cells];
        let mut ctr = vec![Vector3::ZERO; self.n_cells];
        for (fi, f) in self.faces.iter().enumerate() {
            let cf = face_ctr[fi];
            // Owner: Sf already points outward from the owner cell.
            {
                let cell = f.owner;
                let sf = face_sf[fi];
                let pyr3 = sf.dot(cf - c_est[cell]); // 3 * pyramid volume
                let pyr_ctr = cf * 0.75 + c_est[cell] * 0.25;
                vol[cell] += pyr3;
                ctr[cell] += pyr_ctr * pyr3;
            }
            // Neighbour: outward normal is -Sf.
            if let Some(cell) = f.neighbour {
                let sf = face_sf[fi] * -1.0;
                let pyr3 = sf.dot(cf - c_est[cell]);
                let pyr_ctr = cf * 0.75 + c_est[cell] * 0.25;
                vol[cell] += pyr3;
                ctr[cell] += pyr_ctr * pyr3;
            }
        }
        for c in 0..self.n_cells {
            if vol[c].abs() > f64::EPSILON {
                ctr[c] /= vol[c];
            } else {
                ctr[c] = c_est[c];
            }
            vol[c] /= 3.0;
        }
        (vol, ctr)
    }

    /// Convert to the `outram-foam-basic-lib` [`FvMesh`], computing all
    /// finite-volume geometry (cell volumes/centres, face-area vectors, face
    /// areas, face centres) from the point/face topology.
    ///
    /// # Errors
    /// Returns [`MeshError::Construction`] if the assembled mesh fails
    /// `FvMesh::validate` (e.g. non-contiguous patches).
    pub fn to_fv_mesh(&self) -> Result<FvMesh, MeshError> {
        let (cell_volumes, cell_centres) = self.cell_geometry();

        let mut owner = Vec::with_capacity(self.faces.len());
        let mut neighbour = Vec::with_capacity(self.n_internal_faces);
        let mut face_area_vectors = Vec::with_capacity(self.faces.len());
        let mut face_centres = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (c, sf) = face_centre_and_area(&self.points, &f.verts);
            owner.push(f.owner);
            if let Some(nb) = f.neighbour {
                neighbour.push(nb);
            }
            face_area_vectors.push(sf);
            face_centres.push(c);
        }

        FvMeshBuilder::new()
            .n_cells(self.n_cells)
            .n_internal_faces(self.n_internal_faces)
            .owner(owner)
            .neighbour(neighbour)
            .patches(self.patches.clone())
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .map_err(|e| MeshError::Construction(e.to_string()))
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Top-level convenience
// ───────────────────────────────────────────────────────────────────────────

/// Parse a `blockMeshDict` (as text) and build the [`PolyMesh`] in one call.
///
/// Equivalent to `BlockMeshDict::parse(dict_text)?.build()`.
///
/// # Errors
/// [`MeshError::DictParse`] on a malformed dict, [`MeshError::NotImplemented`]
/// for unsupported grading, or [`MeshError::Construction`] on a topological
/// inconsistency.
pub fn block_mesh(dict_text: &str) -> Result<PolyMesh, MeshError> {
    BlockMeshDict::parse(dict_text)?.build()
}

/// Parse a `blockMeshDict` from a file path and build the [`PolyMesh`].
///
/// # Errors
/// As [`block_mesh`], plus [`MeshError::DictParse`] wrapping any I/O error.
pub fn block_mesh_from_file(path: impl AsRef<Path>) -> Result<PolyMesh, MeshError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| MeshError::DictParse(format!("could not read blockMeshDict: {e}")))?;
    block_mesh(&text)
}

// ───────────────────────────────────────────────────────────────────────────
//  Grading
// ───────────────────────────────────────────────────────────────────────────

/// Normalised node positions `[0, 1]` for `n` cells under a `simpleGrading`
/// expansion ratio (last-cell-width / first-cell-width).
///
/// Returns `n + 1` monotonically increasing positions with `s[0] == 0` and
/// `s[n] == 1`. For `expansion == 1` (or `n <= 1`) the spacing is uniform;
/// otherwise cell widths follow a geometric progression whose per-cell ratio
/// `r` satisfies `r^(n-1) == expansion`.
fn graded_positions(n: usize, expansion: f64) -> Vec<f64> {
    if n == 0 {
        return vec![0.0];
    }
    if n == 1 || (expansion - 1.0).abs() < 1e-12 {
        return (0..=n).map(|i| i as f64 / n as f64).collect();
    }
    // Per-cell growth factor r with r^(n-1) = expansion.
    let r = expansion.powf(1.0 / (n as f64 - 1.0));
    // First cell width from the geometric-series sum: w0 (r^n - 1)/(r - 1) = 1.
    let w0 = (r - 1.0) / (r.powi(n as i32) - 1.0);
    let mut s = Vec::with_capacity(n + 1);
    s.push(0.0);
    let mut pos = 0.0;
    let mut w = w0;
    for _ in 0..n {
        pos += w;
        s.push(pos);
        w *= r;
    }
    // Renormalise so the final node lands exactly on 1.0.
    let last = *s.last().unwrap();
    for v in s.iter_mut() {
        *v /= last;
    }
    s
}

/// Normalised node positions `[0, 1]` for `n` cells under a **multi-grading**
/// list (per-direction `simpleGrading` segments).
///
/// Mirrors OpenFOAM's `Foam::lineDivide`
/// (`src/mesh/blockMesh/blockEdges/lineDivide/lineDivide.C`, lines 46–138) and
/// the `gradingDescriptors::normalise` step
/// (`gradingDescriptor/gradingDescriptors.C`, lines 486–503).
///
/// Each [`GradingSegment`] occupies `fraction_length` of the edge and receives
/// `round(fraction_cells * n / sum_cells)` cells, graded geometrically by its
/// `expansion` ratio over that portion. The `fraction_length` /
/// `fraction_cells` are normalised here so each set sums to `1`; the division
/// counts are distributed independently of segment order and the segment with
/// the largest `fraction_cells` absorbs any rounding remainder so the total is
/// exactly `n` (OpenFOAM `secnMaxDivs` adjustment).
///
/// Returns `n + 1` monotonically increasing positions with `s[0] == 0` and
/// `s[n] == 1` (endpoints clamped exactly so block corners coincide across
/// blocks). If `n` is smaller than the number of segments — OpenFOAM cannot
/// place at least one division per section — the edge is meshed uniformly, as
/// OpenFOAM does (`lineDivide.C` line 128).
///
/// Within a segment of `m` cells and per-cell growth `g = expansion^(1/(m-1))`,
/// the cumulative position of the `p`-th interior node is
/// `blockFrac * (1 - g^p) / (1 - g^m)` offset by the segment start — the same
/// geometric-series partial sum used by the single-ratio [`graded_positions`].
fn graded_positions_multi(n: usize, segments: &[GradingSegment]) -> Vec<f64> {
    if n == 0 {
        return vec![0.0];
    }
    // Fewer divisions than sections, or degenerate input: mesh uniformly.
    let sum_len: f64 = segments.iter().map(|s| s.fraction_length).sum();
    let sum_cells: f64 = segments.iter().map(|s| s.fraction_cells).sum();
    if segments.is_empty() || n < segments.len() || sum_len <= 0.0 || sum_cells <= 0.0 {
        return (0..=n).map(|i| i as f64 / n as f64).collect();
    }

    // Distribute divisions to sections (order-independent), rounding to nearest.
    let n_f = n as f64;
    let mut secn_divs: Vec<usize> = segments
        .iter()
        .map(|s| ((s.fraction_cells / sum_cells) * n_f + 0.5) as usize)
        .collect();
    let sum_secn: usize = secn_divs.iter().sum();
    // First section with the largest fraction_cells absorbs the remainder
    // (strict `>` keeps the earliest on ties, matching OpenFOAM `secnMaxDivs`).
    let mut max_i = 0usize;
    for i in 1..segments.len() {
        if segments[i].fraction_cells > segments[max_i].fraction_cells {
            max_i = i;
        }
    }
    if sum_secn != n {
        let adjusted = secn_divs[max_i] as isize + (n as isize - sum_secn as isize);
        // A malformed dict could drive this negative; fall back to uniform.
        if adjusted < 1 {
            return (0..=n).map(|i| i as f64 / n as f64).collect();
        }
        secn_divs[max_i] = adjusted as usize;
    }

    let mut s = vec![0.0f64; n + 1];
    let mut sec_start = 0.0f64;
    let mut secn_start = 1usize; // index of the first node to fill in this section
    for (si, seg) in segments.iter().enumerate() {
        let block_frac = seg.fraction_length / sum_len;
        let expansion = seg.expansion;
        let secn_div = secn_divs[si];
        let secn_end = secn_start + secn_div;
        if (expansion - 1.0).abs() < 1e-12 {
            for (offset, slot) in s[secn_start..secn_end].iter_mut().enumerate() {
                // `p` is the node index within the section (1..=secn_div).
                let p = (offset + 1) as f64;
                *slot = sec_start + block_frac * p / secn_div as f64;
            }
        } else {
            // Per-cell growth factor; OpenFOAM `calcGexp` returns 0 for a single
            // cell, which makes the single-node formula collapse to blockFrac.
            let exp_fact = if secn_div > 1 {
                expansion.powf(1.0 / (secn_div as f64 - 1.0))
            } else {
                0.0
            };
            let denom = 1.0 - exp_fact.powi(secn_div as i32);
            for (offset, slot) in s[secn_start..secn_end].iter_mut().enumerate() {
                let p = (offset + 1) as i32;
                *slot = sec_start + block_frac * (1.0 - exp_fact.powi(p)) / denom;
            }
        }
        sec_start = s[secn_end - 1];
        secn_start = secn_end;
    }
    // Clamp the endpoints exactly so block corners coincide across blocks.
    s[0] = 0.0;
    s[n] = 1.0;
    s
}

/// Trilinear interpolation of the 8 (scaled) block corners at unit-cube
/// parametric coordinates `(u, v, w)`.
fn trilinear(corners: &[Vector3; 8], u: f64, v: f64, w: f64) -> Vector3 {
    let mut p = Vector3::ZERO;
    for (i, c) in corners.iter().enumerate() {
        let [cx, cy, cz] = HEX_CORNER[i];
        let bu = if cx > 0.5 { u } else { 1.0 - u };
        let bv = if cy > 0.5 { v } else { 1.0 - v };
        let bw = if cz > 0.5 { w } else { 1.0 - w };
        p += *c * (bu * bv * bw);
    }
    p
}

// ───────────────────────────────────────────────────────────────────────────
//  Build
// ───────────────────────────────────────────────────────────────────────────

/// A boundary face awaiting patch assignment.
struct PendingBoundaryFace {
    verts: Vec<usize>,
    owner: usize,
    block_face: Option<[usize; 4]>,
}

impl BlockMeshDict {
    /// Subdivide every block, merge coincident points, dedupe faces, assign
    /// boundary patches, and return the [`PolyMesh`].
    ///
    /// # Errors
    /// [`MeshError::Construction`] on a non-manifold face (a face shared by more
    /// than two cells) or a failed validate; boundary faces that match no patch
    /// are collected into a trailing `defaultFaces` patch rather than erroring.
    pub fn build(&self) -> Result<PolyMesh, MeshError> {
        let scale = self.convert_to_meters;

        // Global point merge: quantised coordinate -> canonical point index.
        let tol = self.merge_tolerance();
        let inv_tol = 1.0 / tol;
        let mut point_map: HashMap<(i64, i64, i64), usize> = HashMap::new();
        let mut points: Vec<Vector3> = Vec::new();
        let mut intern = |p: Vector3| -> usize {
            let key = (
                (p.x * inv_tol).round() as i64,
                (p.y * inv_tol).round() as i64,
                (p.z * inv_tol).round() as i64,
            );
            *point_map.entry(key).or_insert_with(|| {
                points.push(p);
                points.len() - 1
            })
        };

        // Candidate faces, keyed later for dedup.
        struct FaceCand {
            verts: [usize; 4],              // wound outward from `owner`
            owner: usize,                   // cell index
            block_face: Option<[usize; 4]>, // sorted coarse block-corner set, if on a block boundary
        }
        let mut cands: Vec<FaceCand> = Vec::new();
        let mut cell_count = 0usize;

        for block in &self.blocks {
            for &vi in &block.vertices {
                if vi >= self.vertices.len() {
                    return Err(MeshError::Construction(format!(
                        "block references vertex {vi} but only {} vertices defined",
                        self.vertices.len()
                    )));
                }
            }
            let corners: [Vector3; 8] = {
                let mut c = [Vector3::ZERO; 8];
                for (k, &vi) in block.vertices.iter().enumerate() {
                    c[k] = self.vertices[vi] * scale;
                }
                c
            };
            let [nx, ny, nz] = block.cells;
            if nx == 0 || ny == 0 || nz == 0 {
                return Err(MeshError::Construction(
                    "block has a zero cell count".to_string(),
                ));
            }
            let us = block.grading[0].node_positions(nx);
            let vs = block.grading[1].node_positions(ny);
            let ws = block.grading[2].node_positions(nz);

            // Node global-index grid for this block.
            let nnx = nx + 1;
            let nny = ny + 1;
            let mut grid = vec![0usize; nnx * nny * (nz + 1)];
            for k in 0..=nz {
                for j in 0..=ny {
                    for i in 0..=nx {
                        let p = trilinear(&corners, us[i], vs[j], ws[k]);
                        grid[i + j * nnx + k * nnx * nny] = intern(p);
                    }
                }
            }
            let node_at = |i: usize, j: usize, k: usize| grid[i + j * nnx + k * nnx * nny];

            // Coarse block-face corner sets (block-corner vertex indices),
            // indexed by HEX_FACE order.
            let block_face_set = |lf: usize| -> [usize; 4] {
                let mut s = [
                    block.vertices[HEX_FACE[lf][0]],
                    block.vertices[HEX_FACE[lf][1]],
                    block.vertices[HEX_FACE[lf][2]],
                    block.vertices[HEX_FACE[lf][3]],
                ];
                s.sort_unstable();
                s
            };

            // Emit cells + their 6 faces.
            for ck in 0..nz {
                for cj in 0..ny {
                    for ci in 0..nx {
                        let cell = cell_count;
                        cell_count += 1;
                        let n = [
                            node_at(ci, cj, ck),
                            node_at(ci + 1, cj, ck),
                            node_at(ci + 1, cj + 1, ck),
                            node_at(ci, cj + 1, ck),
                            node_at(ci, cj, ck + 1),
                            node_at(ci + 1, cj, ck + 1),
                            node_at(ci + 1, cj + 1, ck + 1),
                            node_at(ci, cj + 1, ck + 1),
                        ];
                        let on_boundary = [
                            ci == 0,      // x-min
                            ci == nx - 1, // x-max
                            cj == 0,      // y-min
                            cj == ny - 1, // y-max
                            ck == 0,      // z-min
                            ck == nz - 1, // z-max
                        ];
                        for (lf, corners_lf) in HEX_FACE.iter().enumerate() {
                            let verts = [
                                n[corners_lf[0]],
                                n[corners_lf[1]],
                                n[corners_lf[2]],
                                n[corners_lf[3]],
                            ];
                            let block_face = if on_boundary[lf] {
                                Some(block_face_set(lf))
                            } else {
                                None
                            };
                            cands.push(FaceCand {
                                verts,
                                owner: cell,
                                block_face,
                            });
                        }
                    }
                }
            }
        }

        // Dedup faces by their sorted point set.
        let mut groups: HashMap<[usize; 4], Vec<usize>> = HashMap::new();
        for (idx, c) in cands.iter().enumerate() {
            let mut key = c.verts;
            key.sort_unstable();
            groups.entry(key).or_default().push(idx);
        }

        // Partition into internal faces and pending boundary faces.
        let mut internal: Vec<MeshFace> = Vec::new();
        let mut boundary: Vec<PendingBoundaryFace> = Vec::new();
        for members in groups.values() {
            match members.len() {
                1 => {
                    let c = &cands[members[0]];
                    boundary.push(PendingBoundaryFace {
                        verts: c.verts.to_vec(),
                        owner: c.owner,
                        block_face: c.block_face,
                    });
                }
                2 => {
                    let a = &cands[members[0]];
                    let b = &cands[members[1]];
                    // Owner = smaller cell index; keep that cell's outward winding.
                    let (own, nbr) = if a.owner < b.owner { (a, b) } else { (b, a) };
                    internal.push(MeshFace {
                        verts: own.verts.to_vec(),
                        owner: own.owner,
                        neighbour: Some(nbr.owner),
                    });
                }
                m => {
                    return Err(MeshError::Construction(format!(
                        "non-manifold face shared by {m} cells (points {:?})",
                        cands[members[0]].verts
                    )));
                }
            }
        }

        // Deterministic internal-face ordering: by (owner, neighbour).
        internal.sort_by_key(|f| (f.owner, f.neighbour.unwrap_or(usize::MAX)));

        // Assign boundary faces to patches by coarse block-face set.
        let mut patch_lookup: HashMap<[usize; 4], usize> = HashMap::new();
        for (pi, pd) in self.patches.iter().enumerate() {
            for quad in &pd.faces {
                let mut key = *quad;
                key.sort_unstable();
                patch_lookup.insert(key, pi);
            }
        }
        let mut per_patch: Vec<Vec<PendingBoundaryFace>> =
            (0..self.patches.len()).map(|_| Vec::new()).collect();
        let mut unmatched: Vec<PendingBoundaryFace> = Vec::new();
        for bf in boundary {
            match bf.block_face.and_then(|s| patch_lookup.get(&s).copied()) {
                Some(pi) => per_patch[pi].push(bf),
                None => unmatched.push(bf),
            }
        }

        // Assemble the final face list: internal first, then patches in dict
        // order, then any unmatched faces in a trailing defaultFaces patch.
        let n_internal_faces = internal.len();
        let mut faces = internal;
        let mut patches: Vec<BoundaryPatch> = Vec::new();
        let mut cursor = n_internal_faces;
        for (pi, pd) in self.patches.iter().enumerate() {
            let group = std::mem::take(&mut per_patch[pi]);
            let size = group.len();
            for bf in group {
                faces.push(MeshFace {
                    verts: bf.verts,
                    owner: bf.owner,
                    neighbour: None,
                });
            }
            patches.push(BoundaryPatch::new(pd.name.clone(), cursor, size, pd.kind));
            cursor += size;
        }
        if !unmatched.is_empty() {
            let size = unmatched.len();
            for bf in unmatched {
                faces.push(MeshFace {
                    verts: bf.verts,
                    owner: bf.owner,
                    neighbour: None,
                });
            }
            patches.push(BoundaryPatch::new(
                "defaultFaces",
                cursor,
                size,
                PatchKind::Patch,
            ));
        }

        Ok(PolyMesh {
            points,
            faces,
            n_internal_faces,
            n_cells: cell_count,
            patches,
        })
    }

    /// Absolute coincident-point merge tolerance `[m]`, derived from the scaled
    /// vertex bounding box (`~1e-9` of the diagonal, floored at `1e-12 m`).
    fn merge_tolerance(&self) -> f64 {
        if self.vertices.is_empty() {
            return 1e-9;
        }
        let scale = self.convert_to_meters;
        let mut lo = self.vertices[0] * scale;
        let mut hi = lo;
        for v in &self.vertices {
            let p = *v * scale;
            lo.x = lo.x.min(p.x);
            lo.y = lo.y.min(p.y);
            lo.z = lo.z.min(p.z);
            hi.x = hi.x.max(p.x);
            hi.y = hi.y.max(p.y);
            hi.z = hi.z.max(p.z);
        }
        let diag = (hi - lo).mag();
        (diag * 1e-9).max(1e-12)
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Face geometry (OpenFOAM primitiveMeshFaceCentresAndAreas)
// ───────────────────────────────────────────────────────────────────────────

/// Face centre `[m]` and area vector `[m^2]` for a polygon `verts` (point
/// indices into `points`), by the OpenFOAM triangle-fan decomposition about the
/// vertex average. The area vector follows the winding of `verts`.
fn face_centre_and_area(points: &[Vector3], verts: &[usize]) -> (Vector3, Vector3) {
    let n = verts.len();
    debug_assert!(n >= 3);
    // Triangle case is exact and cheap.
    if n == 3 {
        let a = points[verts[0]];
        let b = points[verts[1]];
        let c = points[verts[2]];
        let centre = (a + b + c) / 3.0;
        let area = (b - a).cross(c - a) * 0.5;
        return (centre, area);
    }
    let mut c_est = Vector3::ZERO;
    for &v in verts {
        c_est += points[v];
    }
    c_est /= n as f64;

    let mut sum_a = 0.0;
    let mut sum_n = Vector3::ZERO;
    let mut sum_ac = Vector3::ZERO;
    for i in 0..n {
        let p1 = points[verts[i]];
        let p2 = points[verts[(i + 1) % n]];
        let mid = (p1 + p2 + c_est) / 3.0;
        // Twice the triangle area vector.
        let n_tri = (p2 - p1).cross(c_est - p1);
        let a_tri = n_tri.mag();
        sum_a += a_tri;
        sum_n += n_tri;
        sum_ac += mid * a_tri;
    }
    let centre = if sum_a > f64::EPSILON {
        sum_ac / sum_a
    } else {
        c_est
    };
    (centre, sum_n * 0.5)
}

// ───────────────────────────────────────────────────────────────────────────
//  Parser
// ───────────────────────────────────────────────────────────────────────────

impl BlockMeshDict {
    /// Parse `blockMeshDict` text into a [`BlockMeshDict`].
    ///
    /// Handles `//` and `/* */` comments, the `FoamFile` header, and the
    /// `convertToMeters`/`scale`, `vertices`, `blocks`, `edges` (skipped),
    /// `boundary`/`patches`, and `mergePatchPairs` (skipped) entries. See the
    /// module docs for deferred features.
    ///
    /// # Errors
    /// [`MeshError::DictParse`] on malformed syntax; [`MeshError::NotImplemented`]
    /// for unsupported grading forms.
    pub fn parse(text: &str) -> Result<Self, MeshError> {
        let tokens = tokenize(text);
        let mut cur = Cursor::new(&tokens);

        let mut convert_to_meters = 1.0;
        let mut vertices = Vec::new();
        let mut blocks = Vec::new();
        let mut patches = Vec::new();

        while let Some(tok) = cur.peek() {
            match tok {
                "convertToMeters" | "scale" => {
                    cur.next();
                    convert_to_meters = cur.parse_f64()?;
                    cur.expect(";")?;
                }
                "vertices" => {
                    cur.next();
                    vertices = parse_vertices(&mut cur)?;
                    cur.expect(";")?;
                }
                "blocks" => {
                    cur.next();
                    blocks = parse_blocks(&mut cur)?;
                    cur.expect(";")?;
                }
                "edges" => {
                    cur.next();
                    cur.skip_parens()?; // straight edges assumed; curved deferred
                    cur.expect(";")?;
                }
                "boundary" | "patches" => {
                    cur.next();
                    patches = parse_boundary(&mut cur)?;
                    cur.expect(";")?;
                }
                "mergePatchPairs" => {
                    cur.next();
                    cur.skip_parens()?;
                    cur.expect(";")?;
                }
                "FoamFile" => {
                    cur.next();
                    cur.skip_braces()?;
                }
                _ => {
                    // Unknown top-level entry: skip its value robustly.
                    cur.next();
                    match cur.peek() {
                        Some("(") => {
                            cur.skip_parens()?;
                            let _ = cur.accept(";");
                        }
                        Some("{") => cur.skip_braces()?,
                        _ => {
                            while let Some(t) = cur.next() {
                                if t == ";" {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if vertices.is_empty() {
            return Err(MeshError::DictParse("no vertices found".into()));
        }
        if blocks.is_empty() {
            return Err(MeshError::DictParse("no blocks found".into()));
        }

        Ok(BlockMeshDict {
            convert_to_meters,
            vertices,
            blocks,
            patches,
        })
    }
}

/// Split dict text into tokens, stripping `//` and `/* */` comments and
/// treating `( ) { } ;` as standalone tokens.
fn tokenize(text: &str) -> Vec<String> {
    // Strip comments first.
    let mut clean = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else {
            clean.push(bytes[i] as char);
            i += 1;
        }
    }

    let mut tokens = Vec::new();
    let mut word = String::new();
    let flush = |word: &mut String, tokens: &mut Vec<String>| {
        if !word.is_empty() {
            tokens.push(std::mem::take(word));
        }
    };
    for ch in clean.chars() {
        match ch {
            '(' | ')' | '{' | '}' | ';' => {
                flush(&mut word, &mut tokens);
                tokens.push(ch.to_string());
            }
            c if c.is_whitespace() => flush(&mut word, &mut tokens),
            c => word.push(c),
        }
    }
    flush(&mut word, &mut tokens);
    tokens
}

/// A forward-only token cursor with small parsing helpers.
struct Cursor<'a> {
    tokens: &'a [String],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [String]) -> Self {
        Self { tokens, pos: 0 }
    }
    fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.pos).map(|s| s.as_str())
    }
    fn next(&mut self) -> Option<&'a str> {
        let t = self.tokens.get(self.pos).map(|s| s.as_str());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn expect(&mut self, tok: &str) -> Result<(), MeshError> {
        match self.next() {
            Some(t) if t == tok => Ok(()),
            other => Err(MeshError::DictParse(format!(
                "expected `{tok}`, found {other:?}"
            ))),
        }
    }
    fn accept(&mut self, tok: &str) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn parse_f64(&mut self) -> Result<f64, MeshError> {
        let t = self
            .next()
            .ok_or_else(|| MeshError::DictParse("expected a number, found end".into()))?;
        t.parse::<f64>()
            .map_err(|_| MeshError::DictParse(format!("expected a number, found `{t}`")))
    }
    fn parse_usize(&mut self) -> Result<usize, MeshError> {
        let t = self
            .next()
            .ok_or_else(|| MeshError::DictParse("expected an integer, found end".into()))?;
        // Accept `20` or `20.0`-style tokens.
        t.parse::<usize>()
            .or_else(|_| t.parse::<f64>().map(|f| f as usize))
            .map_err(|_| MeshError::DictParse(format!("expected an integer, found `{t}`")))
    }
    /// Skip a balanced `( ... )` group (leading `(` must be next).
    fn skip_parens(&mut self) -> Result<(), MeshError> {
        self.expect("(")?;
        let mut depth = 1;
        while depth > 0 {
            match self.next() {
                Some("(") => depth += 1,
                Some(")") => depth -= 1,
                Some(_) => {}
                None => return Err(MeshError::DictParse("unbalanced `(`".into())),
            }
        }
        Ok(())
    }
    /// Skip a balanced `{ ... }` block (leading `{` must be next).
    fn skip_braces(&mut self) -> Result<(), MeshError> {
        self.expect("{")?;
        let mut depth = 1;
        while depth > 0 {
            match self.next() {
                Some("{") => depth += 1,
                Some("}") => depth -= 1,
                Some(_) => {}
                None => return Err(MeshError::DictParse("unbalanced `{`".into())),
            }
        }
        Ok(())
    }
}

/// Parse `( (x y z) (x y z) ... )` into raw (unscaled) points.
fn parse_vertices(cur: &mut Cursor) -> Result<Vec<Vector3>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        cur.expect("(")?;
        let x = cur.parse_f64()?;
        let y = cur.parse_f64()?;
        let z = cur.parse_f64()?;
        cur.expect(")")?;
        out.push(Vector3::new(x, y, z));
    }
    Ok(out)
}

/// Parse the `blocks ( hex (...) (...) simpleGrading (...) ... )` list.
fn parse_blocks(cur: &mut Cursor) -> Result<Vec<Block>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        // Block shape keyword — only `hex` is supported.
        let shape = cur
            .next()
            .ok_or_else(|| MeshError::DictParse("expected `hex`, found end".into()))?;
        if shape != "hex" {
            return Err(MeshError::NotImplemented(format!(
                "block shape `{shape}` (only `hex` is supported)"
            )));
        }
        // 8 vertex indices.
        cur.expect("(")?;
        let mut verts = [0usize; 8];
        for v in verts.iter_mut() {
            *v = cur.parse_usize()?;
        }
        cur.expect(")")?;

        // Optional zone name between the vertex list and the cell counts.
        if cur.peek() != Some("(") {
            cur.next();
        }

        // Cell counts (nx ny nz).
        cur.expect("(")?;
        let nx = cur.parse_usize()?;
        let ny = cur.parse_usize()?;
        let nz = cur.parse_usize()?;
        cur.expect(")")?;

        // Grading.
        let grading_kw = cur
            .next()
            .ok_or_else(|| MeshError::DictParse("expected a grading keyword, found end".into()))?;
        let grading = match grading_kw {
            "simpleGrading" => {
                // Three per-direction gradings, each a scalar ratio or a
                // parenthesised multi-segment list.
                cur.expect("(")?;
                let gx = parse_grading_direction(cur)?;
                let gy = parse_grading_direction(cur)?;
                let gz = parse_grading_direction(cur)?;
                cur.expect(")")?;
                [gx, gy, gz]
            }
            "edgeGrading" => parse_edge_grading(cur)?,
            other => {
                return Err(MeshError::DictParse(format!(
                    "unknown grading keyword `{other}`"
                )));
            }
        };

        out.push(Block {
            vertices: verts,
            cells: [nx, ny, nz],
            grading,
        });
    }
    Ok(out)
}

/// Parse one per-direction grading entry: either a scalar expansion ratio
/// ([`Grading::Uniform`]) or a parenthesised multi-segment list
/// ([`Grading::Multi`]) of `( fractionLength fractionCells expansionRatio )`
/// segments.
///
/// Mirrors OpenFOAM's `operator>>(Istream&, gradingDescriptors&)`
/// (`gradingDescriptor/gradingDescriptors.C`, lines 521–543): a number is a
/// single descriptor, otherwise a list of descriptors is read. A negative
/// expansion ratio is trapped and stored as its inverse
/// (`gradingDescriptor::correct`).
fn parse_grading_direction(cur: &mut Cursor) -> Result<Grading, MeshError> {
    if cur.peek() == Some("(") {
        // Multi-segment list: `( (fl fc er) (fl fc er) ... )`.
        cur.expect("(")?;
        let mut segments = Vec::new();
        while !cur.accept(")") {
            cur.expect("(")?;
            let fraction_length = cur.parse_f64()?;
            let fraction_cells = cur.parse_f64()?;
            let expansion = correct_expansion(cur.parse_f64()?);
            cur.expect(")")?;
            segments.push(GradingSegment {
                fraction_length,
                fraction_cells,
                expansion,
            });
        }
        if segments.is_empty() {
            return Err(MeshError::DictParse(
                "empty multi-grading list `( )`".into(),
            ));
        }
        Ok(Grading::Multi(segments))
    } else {
        Ok(Grading::Uniform(correct_expansion(cur.parse_f64()?)))
    }
}

/// Trap a negative expansion ratio and return its inverse, matching OpenFOAM
/// `gradingDescriptor::correct` (`gradingDescriptor.C`, lines 235–241).
fn correct_expansion(expansion: f64) -> f64 {
    if expansion < 0.0 {
        1.0 / (-expansion)
    } else {
        expansion
    }
}

/// Parse an `edgeGrading ( e0 e1 ... e11 )` list of 12 per-edge gradings.
///
/// OpenFOAM maps the 12 hex edges to the 3 local directions in blocks of four
/// (`blockDescriptorEdges.C`: `sizes()[edgei/4]`, `expand_[edgei]`): edges
/// `0..3` are the x-direction, `4..7` the y-direction, `8..11` the z-direction.
/// A faithful `edgeGrading` grades every edge independently and blends interior
/// nodes from all 12 edge distributions (`blocks/block/blockCreate.C`), which
/// this crate's single-distribution-per-direction block map cannot represent.
///
/// **Restricted port (documented approximation):** if all four edges of a
/// direction share the same [`Grading`], that direction is collapsed to the
/// shared distribution — which is then *exact*, identical to the equivalent
/// `simpleGrading`. If any direction's four edges disagree, the per-edge
/// (non-planar) distribution is genuinely unsupported and this returns
/// [`MeshError::NotImplemented`] naming the offending direction, rather than
/// silently averaging.
fn parse_edge_grading(cur: &mut Cursor) -> Result<[Grading; 3], MeshError> {
    cur.expect("(")?;
    let mut edges: Vec<Grading> = Vec::with_capacity(12);
    while !cur.accept(")") {
        edges.push(parse_grading_direction(cur)?);
        if edges.len() > 12 {
            return Err(MeshError::DictParse(
                "edgeGrading expects exactly 12 edge entries, found more".into(),
            ));
        }
    }
    if edges.len() != 12 {
        return Err(MeshError::DictParse(format!(
            "edgeGrading expects exactly 12 edge entries, found {}",
            edges.len()
        )));
    }

    let dir_name = ["x", "y", "z"];
    let mut out: [Grading; 3] = [
        Grading::Uniform(1.0),
        Grading::Uniform(1.0),
        Grading::Uniform(1.0),
    ];
    for d in 0..3 {
        let base = d * 4;
        let g = &edges[base];
        for e in 1..4 {
            if &edges[base + e] != g {
                return Err(MeshError::NotImplemented(format!(
                    "edgeGrading with differing ratios among the four {}-direction \
                     edges (edges {}..{}): per-edge (non-planar) grading is not \
                     supported; only edgeGrading whose four edges per direction \
                     agree (equivalent to simpleGrading) is handled",
                    dir_name[d],
                    base,
                    base + 3
                )));
            }
        }
        out[d] = g.clone();
    }
    Ok(out)
}

/// Parse the `boundary ( name { type t; faces ( ... ); } ... )` list.
fn parse_boundary(cur: &mut Cursor) -> Result<Vec<PatchDef>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        let name = cur
            .next()
            .ok_or_else(|| MeshError::DictParse("expected a patch name, found end".into()))?
            .to_string();
        cur.expect("{")?;
        let mut kind = PatchKind::Patch;
        let mut faces = Vec::new();
        loop {
            match cur.peek() {
                Some("}") => {
                    cur.next();
                    break;
                }
                Some("type") => {
                    cur.next();
                    let t = cur
                        .next()
                        .ok_or_else(|| MeshError::DictParse("expected a patch type".into()))?;
                    kind = patch_kind_from_str(t);
                    cur.expect(";")?;
                }
                Some("faces") => {
                    cur.next();
                    faces = parse_face_list(cur)?;
                    cur.expect(";")?;
                }
                Some(_) => {
                    // Unknown key inside a patch: skip its value.
                    cur.next();
                    match cur.peek() {
                        Some("(") => {
                            cur.skip_parens()?;
                            let _ = cur.accept(";");
                        }
                        Some("{") => cur.skip_braces()?,
                        _ => {
                            while let Some(t) = cur.next() {
                                if t == ";" {
                                    break;
                                }
                            }
                        }
                    }
                }
                None => return Err(MeshError::DictParse("unterminated patch block".into())),
            }
        }
        out.push(PatchDef { name, kind, faces });
    }
    Ok(out)
}

/// Parse `( (i j k l) (i j k l) ... )` face quads.
fn parse_face_list(cur: &mut Cursor) -> Result<Vec<[usize; 4]>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        cur.expect("(")?;
        let mut quad = [0usize; 4];
        for q in quad.iter_mut() {
            *q = cur.parse_usize()?;
        }
        cur.expect(")")?;
        out.push(quad);
    }
    Ok(out)
}

/// Map an OpenFOAM patch `type` keyword to a [`PatchKind`].
fn patch_kind_from_str(t: &str) -> PatchKind {
    match t {
        "wall" => PatchKind::Wall,
        "empty" => PatchKind::Empty,
        "symmetry" | "symmetryPlane" => PatchKind::Symmetry,
        "wedge" => PatchKind::Wedge,
        "cyclic" | "cyclicAMI" => PatchKind::Cyclic,
        "processor" => PatchKind::Processor,
        _ => PatchKind::Patch,
    }
}

// ───────────────────────────────────────────────────────────────────────────
//  Tests
// ───────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit-cube `blockMeshDict` (`[0,1]^3`, `convertToMeters 1`) whose
    /// x-direction uses the multi-grading list `((0.2 0.3 4)(0.6 0.4 3)(0.2 0.3
    /// 1))` and whose y/z are uniform. `nx` x-cells, 1 cell in y and z.
    fn multigrading_cube_dict(nx: usize) -> String {
        format!(
            "convertToMeters 1;\n\
             vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );\n\
             blocks ( hex (0 1 2 3 4 5 6 7) ({nx} 1 1) \
             simpleGrading ( ((0.2 0.3 4)(0.6 0.4 3)(0.2 0.3 1)) 1 1 ) );\n\
             boundary ( );\n"
        )
    }

    /// V&V — multi-grading node distribution (`graded_positions_multi`).
    ///
    /// Methodology: distribute `n = 10` cells over the three segments
    /// `(0.2,0.3,4) (0.6,0.4,3) (0.2,0.3,1)` (fractionLength, fractionCells,
    /// expansion). Reference is OpenFOAM `lineDivide` (`lineDivide.C:46-138`):
    /// cell counts `round(fractionCells/sumCells * n)` with the largest segment
    /// absorbing the remainder, geometric spacing per segment. Pass criteria:
    /// exactly 11 monotone nodes on `[0,1]`; segment boundaries land on the
    /// cumulative fractionLengths `0.2` and `0.8`; segment-0 cell widths follow
    /// the requested end/start ratio of 4 (per-cell ratio `4^(1/2)=2`).
    ///
    /// Results (measured 2026-07-17): division counts `[3,4,3]`; nodes
    /// `[0, 0.028571, 0.085714, 0.2, 0.279762, 0.3948, 0.560713, 0.8,
    /// 0.866667, 0.933333, 1.0]`; boundary nodes at index 3 (`x=0.2`) and 7
    /// (`x=0.8`); segment-0 widths `[0.028571, 0.057143, 0.114286]` giving
    /// successive ratios `2.0, 2.0` and an end/start of `4.0`; segment-2 widths
    /// all `0.066667` (uniform, expansion 1). All criteria met.
    #[test]
    fn multi_grading_positions() {
        let segs = vec![
            GradingSegment {
                fraction_length: 0.2,
                fraction_cells: 0.3,
                expansion: 4.0,
            },
            GradingSegment {
                fraction_length: 0.6,
                fraction_cells: 0.4,
                expansion: 3.0,
            },
            GradingSegment {
                fraction_length: 0.2,
                fraction_cells: 0.3,
                expansion: 1.0,
            },
        ];
        let s = graded_positions_multi(10, &segs);

        // 11 nodes, exact endpoints.
        assert_eq!(s.len(), 11);
        assert!((s[0] - 0.0).abs() < 1e-12);
        assert!((s[10] - 1.0).abs() < 1e-12);

        // Strictly monotone increasing.
        for i in 1..s.len() {
            assert!(s[i] > s[i - 1], "not monotone at node {i}: {s:?}");
        }

        // Segment boundaries at cumulative fractionLengths 0.2 and 0.8
        // (division counts [3,4,3] -> boundary node indices 3 and 7).
        assert!((s[3] - 0.2).abs() < 1e-9, "seg0/1 boundary = {}", s[3]);
        assert!((s[7] - 0.8).abs() < 1e-9, "seg1/2 boundary = {}", s[7]);

        // Segment-0 widths follow per-cell ratio 2.0 (end/start expansion 4).
        let w: Vec<f64> = (0..10).map(|i| s[i + 1] - s[i]).collect();
        assert!((w[1] / w[0] - 2.0).abs() < 1e-9, "seg0 ratio1 = {}", w[1] / w[0]);
        assert!((w[2] / w[1] - 2.0).abs() < 1e-9, "seg0 ratio2 = {}", w[2] / w[1]);
        assert!((w[2] / w[0] - 4.0).abs() < 1e-9, "seg0 end/start = {}", w[2] / w[0]);

        // Segment-2 is uniform (expansion 1): equal widths.
        assert!((w[8] - w[7]).abs() < 1e-9 && (w[9] - w[7]).abs() < 1e-9);
    }

    /// V&V — simple (single-ratio) grading, no regression.
    ///
    /// Methodology: a uniform expansion ratio of 1 must give equal cell sizes,
    /// and the [`Grading::Uniform`] path must agree bit-for-bit with the legacy
    /// [`graded_positions`] helper. Reference: exact even division.
    /// Pass criterion: nodes `i/n`.
    ///
    /// Results (measured 2026-07-17): `Grading::Uniform(1.0).node_positions(4)`
    /// = `[0, 0.25, 0.5, 0.75, 1.0]`; cell widths all `0.25`. Passed.
    #[test]
    fn simple_grading_uniform_regression() {
        let s = Grading::Uniform(1.0).node_positions(4);
        assert_eq!(s, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        // node_positions dispatches to graded_positions for the Uniform case.
        assert_eq!(s, graded_positions(4, 1.0));
        for i in 0..4 {
            assert!(((s[i + 1] - s[i]) - 0.25).abs() < 1e-12);
        }
    }

    /// V&V — a single-segment multi-grading equals the scalar simpleGrading.
    ///
    /// Methodology: multi-grading with one segment `(1,1,r)` must reproduce the
    /// scalar `graded_positions(n, r)` (both are the same geometric series).
    /// Reference: [`graded_positions`]. Pass criterion: max node difference
    /// `< 1e-12`.
    ///
    /// Results (measured 2026-07-17): for `r=5, n=8` the two distributions
    /// agree to `< 1e-12` at every node. Passed.
    #[test]
    fn single_segment_matches_scalar() {
        let seg = vec![GradingSegment {
            fraction_length: 1.0,
            fraction_cells: 1.0,
            expansion: 5.0,
        }];
        let a = graded_positions_multi(8, &seg);
        let b = graded_positions(8, 5.0);
        for i in 0..a.len() {
            assert!((a[i] - b[i]).abs() < 1e-12, "node {i}: {} vs {}", a[i], b[i]);
        }
    }

    /// V&V — the parser produces the right [`Grading`] variants.
    ///
    /// Methodology: parse [`multigrading_cube_dict`] and inspect the block's
    /// grading. Pass criterion: x is `Multi` with the three parsed segments;
    /// y and z are `Uniform(1.0)`; cell counts `(10,1,1)`.
    ///
    /// Results (measured 2026-07-17): x = `Multi([(0.2,0.3,4),(0.6,0.4,3),
    /// (0.2,0.3,1)])`, y = z = `Uniform(1.0)`. Passed.
    #[test]
    fn parse_multi_grading() {
        let dict = BlockMeshDict::parse(&multigrading_cube_dict(10)).expect("parse");
        assert_eq!(dict.blocks[0].cells, [10, 1, 1]);
        match &dict.blocks[0].grading[0] {
            Grading::Multi(segs) => {
                assert_eq!(segs.len(), 3);
                assert_eq!(segs[0].fraction_length, 0.2);
                assert_eq!(segs[0].fraction_cells, 0.3);
                assert_eq!(segs[0].expansion, 4.0);
                assert_eq!(segs[1].expansion, 3.0);
                assert_eq!(segs[2].expansion, 1.0);
            }
            other => panic!("expected Multi, got {other:?}"),
        }
        assert_eq!(dict.blocks[0].grading[1], Grading::Uniform(1.0));
        assert_eq!(dict.blocks[0].grading[2], Grading::Uniform(1.0));
    }

    /// V&V — a multi-graded cube builds a valid mesh whose volume is preserved.
    ///
    /// Methodology: build the mesh from [`multigrading_cube_dict`] (`10x1x1`),
    /// convert to [`FvMesh`], and check topology + geometry. Reference: the
    /// analytic block volume `1 x 1 x 1 = 1 m^3`; grading only moves node
    /// positions, never the cell count or total volume. Pass criteria: 10
    /// cells; `FvMesh::validate` passes; total volume `= 1 m^3`; the built mesh
    /// carries interior node planes at `x = 0.2 m` and `x = 0.8 m` (the segment
    /// boundaries).
    ///
    /// Results (measured 2026-07-17): 10 cells; validate OK; total volume
    /// `1.0 m^3` (error `< 1e-12`); unique x-planes include `0.2` and `0.8`.
    /// Passed.
    #[test]
    fn multi_grading_build_volume() {
        let mesh = block_mesh(&multigrading_cube_dict(10)).expect("build");
        assert_eq!(mesh.n_cells, 10);

        // Total volume equals the analytic block volume.
        assert!(
            (mesh.total_volume() - 1.0).abs() < 1e-12,
            "volume = {}",
            mesh.total_volume()
        );

        // FvMesh assembles and validates.
        let fv = mesh.to_fv_mesh().expect("to_fv_mesh");
        assert!(fv.validate().is_ok());

        // The graded x node-planes land at the segment boundaries 0.2 and 0.8.
        let mut xs: Vec<f64> = mesh.points.iter().map(|p| p.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(xs.len(), 11, "expected 11 x-planes, got {xs:?}");
        assert!(xs.iter().any(|&x| (x - 0.2).abs() < 1e-9));
        assert!(xs.iter().any(|&x| (x - 0.8).abs() < 1e-9));
    }

    /// V&V — restricted `edgeGrading` with matching edges is exact.
    ///
    /// Methodology: an `edgeGrading` whose four x-edges all use ratio 2 and
    /// whose y/z edges are all 1 must collapse to `simpleGrading (2 1 1)`. Build
    /// a `4x1x1` unit cube and compare against the equivalent `simpleGrading`
    /// mesh. Reference: OpenFOAM edge→direction mapping (edges 0..3 = x). Pass
    /// criteria: parses to `Uniform(2)/Uniform(1)/Uniform(1)`; identical node
    /// planes to `simpleGrading`; volume `= 1 m^3`; validate passes.
    ///
    /// Results (measured 2026-07-17): grading = `[Uniform(2), Uniform(1),
    /// Uniform(1)]`; x-planes identical to `simpleGrading (2 1 1)` within
    /// `1e-12`; total volume `1.0 m^3`; validate OK. Passed.
    #[test]
    fn edge_grading_equal_edges() {
        let verts =
            "vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );";
        let edge_dict = format!(
            "convertToMeters 1;\n{verts}\n\
             blocks ( hex (0 1 2 3 4 5 6 7) (4 1 1) \
             edgeGrading ( 2 2 2 2  1 1 1 1  1 1 1 1 ) );\nboundary ( );\n"
        );
        let simple_dict = format!(
            "convertToMeters 1;\n{verts}\n\
             blocks ( hex (0 1 2 3 4 5 6 7) (4 1 1) simpleGrading (2 1 1) );\nboundary ( );\n"
        );

        let dict = BlockMeshDict::parse(&edge_dict).expect("parse edgeGrading");
        assert_eq!(dict.blocks[0].grading[0], Grading::Uniform(2.0));
        assert_eq!(dict.blocks[0].grading[1], Grading::Uniform(1.0));
        assert_eq!(dict.blocks[0].grading[2], Grading::Uniform(1.0));

        let m_edge = block_mesh(&edge_dict).expect("build edge");
        let m_simple = block_mesh(&simple_dict).expect("build simple");
        assert!((m_edge.total_volume() - 1.0).abs() < 1e-12);
        assert!(m_edge.to_fv_mesh().expect("fv").validate().is_ok());

        let planes = |m: &PolyMesh| -> Vec<f64> {
            let mut xs: Vec<f64> = m.points.iter().map(|p| p.x).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            xs
        };
        let pe = planes(&m_edge);
        let ps = planes(&m_simple);
        assert_eq!(pe.len(), ps.len());
        for (a, b) in pe.iter().zip(ps.iter()) {
            assert!((a - b).abs() < 1e-12, "edge {a} vs simple {b}");
        }
    }

    /// V&V — genuinely per-edge `edgeGrading` is honestly rejected.
    ///
    /// Methodology: an `edgeGrading` whose four x-edges disagree (2,2,3,2)
    /// cannot be represented by one distribution per direction, so the parser
    /// must return [`MeshError::NotImplemented`] rather than silently averaging.
    /// Pass criterion: `parse` returns `NotImplemented` mentioning the
    /// x-direction.
    ///
    /// Results (measured 2026-07-17): `NotImplemented` returned, message names
    /// the x-direction edges. Passed.
    #[test]
    fn edge_grading_differing_edges_not_implemented() {
        let dict = "convertToMeters 1;\n\
             vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );\n\
             blocks ( hex (0 1 2 3 4 5 6 7) (4 1 1) \
             edgeGrading ( 2 2 3 2  1 1 1 1  1 1 1 1 ) );\nboundary ( );\n";
        match BlockMeshDict::parse(dict) {
            Err(MeshError::NotImplemented(msg)) => {
                assert!(msg.contains("x-direction"), "message: {msg}");
            }
            other => panic!("expected NotImplemented, got {other:?}"),
        }
    }

    /// V&V — negative expansion ratios are trapped as their inverse.
    ///
    /// Methodology: OpenFOAM `gradingDescriptor::correct` treats a negative
    /// expansion ratio `-r` as `1/r`. Parse `simpleGrading (-2 1 1)` and check.
    /// Pass criterion: x grading is `Uniform(0.5)`.
    ///
    /// Results (measured 2026-07-17): x = `Uniform(0.5)`. Passed.
    #[test]
    fn negative_expansion_trapped() {
        let dict = "convertToMeters 1;\n\
             vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );\n\
             blocks ( hex (0 1 2 3 4 5 6 7) (4 1 1) simpleGrading (-2 1 1) );\nboundary ( );\n";
        let parsed = BlockMeshDict::parse(dict).expect("parse");
        assert_eq!(parsed.blocks[0].grading[0], Grading::Uniform(0.5));
    }
}
