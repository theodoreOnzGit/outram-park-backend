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
//! 2. [`BlockMeshDict::build`] generates every block's points (12-edge blend),
//!    merges points **topologically** across the block faces the block topology
//!    shares, numbers points, faces and cells as `blockMesh` does (gh:#1464),
//!    checks the topology, and returns a [`PolyMesh`]. ~~merges globally
//!    coincident points by quantised coordinates~~ **CORRECTED 2026-11-11
//!    (gh:#1464)**: coordinates play no part in the merge except to pair
//!    points on a shared block face, exactly as upstream's `calcMergeInfo`;
//!    blocks that only touch geometrically are not merged.
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
//! - ~~`edges` blocks (arc / spline / polyLine) are parsed and **skipped**: all
//!   block edges are treated as straight lines~~ **CORRECTED 2026-11-11
//!   (gh:#1461)**: `edges` (`line`, `arc` in both forms, `polyLine`, `spline`,
//!   `BSpline`) are parsed and applied, see [`crate::block_edges`]; a block
//!   with a listed edge takes the full 12-edge blend plus upstream's
//!   curved-edge correction. ~~`project` / `projectCurve` edges, `geometry`,
//!   `faces`~~ **CORRECTED 2026-11-11 (gh:#1462)**: `geometry` (sphere,
//!   cylinder, plane), `project` vertices, `project` / `projectCurve` edges and
//!   `project` / `sweep` block faces are ported, see
//!   [`crate::block_projection`]; a surface type that is not ported (including
//!   `triSurface`, `extrudedCircle`, `plate`) returns
//!   [`MeshError::NotImplemented`]. `#calc` / `#include` and `namedBlocks`
//!   return [`MeshError::NotImplemented`]; they are never skipped.
//! - `units [mm]` (a single length unit), `convertToMeters` and `scale` are
//!   read, at most one of the three (gh:#1463). `defaultPatch { name; type; }`
//!   is read; without it the unassigned faces form `defaultFaces` of type
//!   `empty`, as upstream (it was `patch` before 2026-11-11). `$name` of an
//!   earlier top-level entry is expanded. Block zone names are kept in
//!   [`PolyMesh::cell_zones`]; the `blockMesh` CLI writes them as
//!   `constant/polyMesh/cellZones` (`io::PolyMesh::write_cell_zones`).
//! - `simpleGrading (gx gy gz)` is fully supported, where each of the three
//!   directions is **either** a single scalar expansion ratio **or** a
//!   multi-grading list of `( fractionLength fractionCells expansionRatio )`
//!   segments (OpenFOAM `gradingDescriptors`), graded piecewise-geometrically.
//! - `edgeGrading (e0 … e11)` is **fully supported**: each of the hex's 12
//!   edges carries its own [`Grading`] (single ratio or multi-segment), and
//!   interior nodes are blended from all 12 edge distributions exactly as
//!   OpenFOAM's `block::createPoints` does (the straight-edge, no-curvature
//!   case). ~~When the four edges of a direction happen to agree, the block
//!   collapses to the equivalent `simpleGrading` fast path (bit-identical to a
//!   single per-direction distribution)~~ **CORRECTED 2026-11-11 (gh:#1464):**
//!   every block, `simpleGrading` or not, takes the 12-edge blend, as upstream
//!   (`blockCreate.C` has no trilinear path); the trilinear map differed from
//!   it by round-off. See `edge_blended_node_with`.
//! - ~~`mergePatchPairs` is parsed and ignored~~ **CORRECTED 2026-11-11**: a
//!   non-empty `mergePatchPairs` returns [`MeshError::NotImplemented`]
//!   (gh:#1464). `fastMerge yes` also returns `NotImplemented`
//!   (`blockMeshMergeFast.C`).
//!
//! ## Honest scope — what the per-edge blend does NOT model
//!
//! The straight-edge branch and, since 2026-11-11 (gh:#1461), the curved-edge
//! correction of OpenFOAM `block::createPoints` (`blockCreate.C:45-183`) are
//! ported. ~~The curved-**face** projection passes (`blockCreate.C:185-345`)
//! are **not** implemented (gh:#1462)~~ **CORRECTED 2026-11-11 (gh:#1462)**:
//! they are, in [`crate::block_projection::apply_curved_faces`]; a block with no
//! `faces` entry never enters them. For blocks with planar faces (straight or
//! curved edges only) the
//! blend is exact and volume-preserving; a warped internal face between cells of
//! a per-edge-graded block is expected and handled by the divergence-theorem
//! cell geometry. For multi-block meshes, matching gradings on any **shared**
//! edge remain the dict author's responsibility, exactly as in OpenFOAM
//! `blockMesh` (a mismatch leaves the shared boundary nodes un-merged).

use petir::mathf::RealMath;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;

use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::block_edges::{BlockEdge, BlockEdgeKind, EdgeCurve};
use crate::block_projection::{
    apply_curved_faces, find_curved_faces, read_dict_body, BlockFace, BlockFaceKind, Geometry,
    ProjectionFlavour,
};
use crate::block_topology::{
    calc_merge_info, create_cells, create_patch_faces, create_points, faces_equal, mesh_from_shapes,
    BlockGrid, CellModel, CellShape, Face, ShapeMesh,
};
use crate::MeshError;

// ───────────────────────────────────────────────────────────────────────────
//  Hex topology constants (OpenFOAM `hex` cellModel)
// ───────────────────────────────────────────────────────────────────────────

/// Unit-cube coordinates of the 8 hex vertices, in OpenFOAM local ordering.
///
/// `v0=(0,0,0) v1=(1,0,0) v2=(1,1,0) v3=(0,1,0) v4=(0,0,1) v5=(1,0,1)
///  v6=(1,1,1) v7=(0,1,1)`.
#[cfg(test)]
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
    #[cfg(test)]
    fn node_positions(&self, n: usize) -> Vec<f64> {
        line_divide_weights(n, self, ProjectionFlavour::Dev)
    }
}

impl Grading {
    /// The grading as `gradingDescriptors` segments (a scalar ratio is one
    /// segment covering the whole edge).
    fn as_segments(&self) -> Vec<GradingSegment> {
        match self {
            Grading::Uniform(r) => vec![GradingSegment {
                fraction_length: 1.0,
                fraction_cells: 1.0,
                expansion: *r,
            }],
            Grading::Multi(s) => s.clone(),
        }
    }

    /// `gradingDescriptors::inv()`: the segments in reverse order, each
    /// expansion ratio inverted (the grading seen from the other end).
    fn inv(&self) -> Grading {
        let mut segs = self.as_segments();
        segs.reverse();
        for s in segs.iter_mut() {
            s.expansion = 1.0 / s.expansion;
        }
        Grading::Multi(segs)
    }
}

/// `lineDivide::lambdaDivisions()` for `n` divisions under `grading`: always
/// the `lineDivide.C` formulas (a scalar ratio is a single descriptor), which
/// is what curved edges use (`blockDescriptorEdges.C`). `flavour` selects the
/// OpenFOAM line followed where dev and v2506 differ (see
/// [`graded_positions_multi`]).
fn line_divide_weights(n: usize, grading: &Grading, flavour: ProjectionFlavour) -> Vec<f64> {
    graded_positions_multi(n, &grading.as_segments(), flavour)
}

/// OpenFOAM-dev's `pow(Scalar, int32_t)`: `IntegerPow`
/// (`src/OpenFOAM/primitives/Scalar/Scalar.H`, 852527377c7e), exponentiation by
/// repeated squaring in upstream's exact order (`e == 0`, `1`, `-1` special
/// cases; `xx` is `x` or `1/x`; `y *= xx` on odd `i`, `xx *= xx` each step).
/// The multiplication order, not the mathematical value, is what makes the
/// result bit-identical to the dev binary; the real `pow` differs by up to
/// 1 ulp.
fn integer_pow_dev(x: f64, e: i32) -> f64 {
    if e == 0 {
        return 1.0;
    } else if e == 1 {
        return x;
    } else if e == -1 {
        return 1.0 / x;
    }
    let mut xx = if e > 0 { x } else { 1.0 / x };
    let mut y = 1.0;
    let mut i = e;
    while i != 0 {
        if i % 2 != 0 {
            y *= xx;
        }
        xx *= xx;
        i /= 2;
    }
    y
}

/// `pow(expFact, int)` of `lineDivide.C`, per flavour. Dev: [`integer_pow_dev`].
/// v2506: the real power (`pow(double, double)` after integer promotion).
fn line_divide_pow(base: f64, e: usize, flavour: ProjectionFlavour) -> f64 {
    match flavour {
        ProjectionFlavour::Dev => integer_pow_dev(base, e as i32),
        ProjectionFlavour::Esi => base.r_powf(e as f64),
    }
}

/// Local hex-edge endpoints `(start, end)` in OpenFOAM edge order
/// (`blockDescriptorEdges.C`, `edgesPointsWeights`).
const HEX_EDGE_ENDS: [(usize, usize); 12] = [
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

/// One `hex` block from the `blocks` list.
///
/// The block is a hexahedron whose 8 corners are indices into
/// [`BlockMeshDict::vertices`], subdivided into `cells[0] * cells[1] * cells[2]`
/// hexahedral cells. `grading` holds the three per-direction [`Grading`]
/// distributions (from `simpleGrading` or, when all four edges of a direction
/// agree, `edgeGrading`); each direction only redistributes node positions
/// along its `[0, 1]` parametric edge (`1.0` uniform expansion is even
/// spacing).
///
/// `edge_grading` is the genuine per-edge case: `Some([Grading; 12])` when an
/// `edgeGrading` entry gives four edges of some direction *different* gradings,
/// so interior nodes must be blended from all 12 edge distributions
/// (OpenFOAM `block::createPoints`) rather than one distribution per direction.
/// When it is `Some`, the block-build path uses `edge_blended_node` and
/// `grading` merely holds a representative (the first edge of each direction)
/// for inspection. When it is `None`, `grading` is authoritative and each of a
/// direction's four edges takes it (`simpleGrading`, and every `edgeGrading`
/// whose four edges per direction agree). ~~`None` took a trilinear fast
/// path~~ **CORRECTED 2026-11-11 (gh:#1464):** both cases take the 12-edge blend.
#[derive(Debug, Clone)]
pub struct Block {
    /// The 8 block-corner vertex indices, in OpenFOAM hex order.
    pub vertices: [usize; 8],
    /// Cell counts `(nx, ny, nz)` along the three local directions.
    pub cells: [usize; 3],
    /// Per-direction node distributions `(gx, gy, gz)`. Authoritative when
    /// `edge_grading` is `None`; a per-direction representative otherwise.
    pub grading: [Grading; 3],
    /// The 12 per-edge gradings, present only for a genuinely per-edge
    /// `edgeGrading` (edges 0–3 = x, 4–7 = y, 8–11 = z; see
    /// `edge_blended_node`). `None`: each direction's four edges share `grading`.
    pub edge_grading: Option<[Grading; 12]>,
    /// The block's zone name (`hex (...) zoneName (nx ny nz) ...`), which
    /// upstream turns into a `cellZone`; `None` when the block is unzoned.
    pub zone: Option<String>,
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
    /// The `edges` list: curved (or explicitly `line`) block edges, in
    /// dictionary order (the first match wins, as upstream's
    /// `blockDescriptor::edgePointsWeights`). Empty means every edge is straight.
    pub edges: Vec<BlockEdge>,
    /// Name of the patch that collects block faces no `boundary` entry names
    /// (`defaultPatch { name ...; }`, default `defaultFaces`).
    pub default_patch_name: String,
    /// Type of that patch (`defaultPatch { type ...; }`). Upstream's default
    /// when the entry is absent is `empty` (`blockMeshTopology.C`,
    /// `defaultPatchType = emptyPolyPatch::typeName`).
    pub default_patch_kind: PatchKind,
    /// `fastMerge` (default `no`). `yes` selects `blockMeshMergeFast.C`, which
    /// is not ported: `build` returns `NotImplemented` for it.
    pub fast_merge: bool,
    /// `checkFaceCorrespondence` (default `yes`): neighbouring blocks must
    /// have the same number of faces on their shared block face.
    pub check_face_correspondence: bool,
    /// `checkBlockFaceOrientation` (default `yes`): every block's faces must
    /// point away from its centre (`blockDescriptor::check`).
    pub check_block_face_orientation: bool,
    /// The `geometry` dictionary: searchable surfaces that `project` vertices,
    /// edges and faces refer to (gh:#1462). Empty when absent.
    pub geometry: Arc<Geometry>,
    /// The `faces` list: `project` and `sweep` block faces (gh:#1462).
    pub faces: Vec<BlockFace>,
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
    /// Cell zones, one per distinct block zone name in dictionary order:
    /// `(name, cell indices)`. Upstream writes these as `cellZones`; the
    /// `io::PolyMesh` carries no zones, so [`PolyMesh::to_foam_poly_mesh`]
    /// drops them; write them with
    /// `outram_foam_basic_lib::io::poly_mesh::PolyMesh::write_cell_zones`
    /// (~~"no zone output yet"~~ **CORRECTED 2026-11-11, gh:#1463**).
    pub cell_zones: Vec<(String, Vec<usize>)>,
}

impl PolyMesh {
    /// Several region meshes side by side as one `PolyMesh`, for drawing a
    /// multi-region case (gh:#1156): nothing is merged or connected, the
    /// regions' cells are numbered in order (region `k`'s cell `c` becomes
    /// `c + Σ_{j<k} n_cells_j`), every internal face comes first, then each
    /// region's patches, renamed `"<region>/<patch>"`. A solver never sees
    /// this mesh; each region keeps its own.
    pub fn concatenate(parts: &[(&str, &PolyMesh)]) -> PolyMesh {
        let mut points = Vec::new();
        let mut internal = Vec::new();
        let mut boundary: Vec<(String, outram_foam_basic_lib::mesh::PatchKind, Vec<MeshFace>)> = Vec::new();
        let (mut p0, mut c0) = (0, 0);
        for (name, m) in parts {
            let shift = |f: &MeshFace| MeshFace {
                verts: f.verts.iter().map(|v| v + p0).collect(),
                owner: f.owner + c0,
                neighbour: f.neighbour.map(|n| n + c0),
            };
            points.extend_from_slice(&m.points);
            internal.extend(m.faces[..m.n_internal_faces].iter().map(shift));
            for p in &m.patches {
                boundary.push((format!("{name}/{}", p.name), p.kind, m.faces[p.start..p.start + p.size].iter().map(shift).collect()));
            }
            p0 += m.points.len();
            c0 += m.n_cells;
        }
        let n_internal_faces = internal.len();
        let mut faces = internal;
        let mut patches = Vec::new();
        for (name, kind, fs) in boundary {
            patches.push(BoundaryPatch::new(name, faces.len(), fs.len(), kind));
            faces.extend(fs);
        }
        PolyMesh { points, faces, n_internal_faces, n_cells: c0, patches, cell_zones: Vec::new() }
    }

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

    /// Convert to the writable `outram-foam-basic-lib`
    /// [`PolyMesh`](outram_foam_basic_lib::io::PolyMesh) — the on-disk
    /// `constant/polyMesh` representation.
    ///
    /// A pure re-packing of the identical data (same face ordering, same
    /// winding convention, same patch layout); no geometry is recomputed. Use
    /// it to write a `blockMesh` result to an OpenFOAM case with
    /// [`PolyMesh::write`](outram_foam_basic_lib::io::PolyMesh::write), or to
    /// grade it with [`assess_quality`](crate::mesh_quality::assess_quality).
    ///
    /// ```no_run
    /// let mesh = outram_foam_mesh::block_mesh::block_mesh_from_file("blockMeshDict").unwrap();
    /// mesh.to_foam_poly_mesh().write("case/constant/polyMesh").unwrap();
    /// ```
    pub fn to_foam_poly_mesh(&self) -> outram_foam_basic_lib::io::PolyMesh {
        let faces = self
            .faces
            .iter()
            .map(|f| outram_foam_basic_lib::io::MeshFace {
                verts: f.verts.clone(),
                owner: f.owner,
                neighbour: f.neighbour,
            })
            .collect();
        outram_foam_basic_lib::io::PolyMesh {
            points: self.points.clone(),
            faces,
            n_internal_faces: self.n_internal_faces,
            n_cells: self.n_cells,
            patches: self.patches.clone(),
        }
    }

    /// The inverse of [`PolyMesh::to_foam_poly_mesh`]: take an
    /// `outram-foam-basic-lib` polyMesh (read from disk, or converted from
    /// another mesher, e.g. the cfMesh port through `outram_blender`'s neutral
    /// mesh, gh:#1202) as this crate's `PolyMesh`, so the section cutter and
    /// [`PolyMesh::to_fv_mesh`] work on it. A pure re-packing, like its inverse.
    pub fn from_foam_poly_mesh(m: &outram_foam_basic_lib::io::PolyMesh) -> PolyMesh {
        PolyMesh {
            points: m.points.clone(),
            faces: m.faces.iter().map(|f| MeshFace { verts: f.verts.clone(), owner: f.owner, neighbour: f.neighbour }).collect(),
            n_internal_faces: m.n_internal_faces,
            n_cells: m.n_cells,
            patches: m.patches.clone(),
            cell_zones: Vec::new(),
        }
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
/// for an unsupported block shape (only `hex` is handled; all grading forms —
/// `simpleGrading`, multi-grading, and full per-edge `edgeGrading` — are
/// supported), or [`MeshError::Construction`] on a topological inconsistency.
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

#[cfg(test)]
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
    let r = expansion.r_powf(1.0 / (n as f64 - 1.0));
    // First cell width from the geometric-series sum: w0 (r^n - 1)/(r - 1) = 1.
    let w0 = (r - 1.0) / (r.r_powi(n as i32) - 1.0);
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
/// Returns `n + 1` monotonically increasing positions with `s[0] == 0`.
/// ~~`s[n]` was clamped to exactly `1`~~ **CORRECTED 2026-11-11 (gh:#1364):**
/// neither dev nor v2506 clamps: `lineDivide.C` sets `divisions_[nDiv] = 1.0`
/// and the last section's loop then overwrites it with the computed value
/// (`secStart + blockFrac*...`, 1 ulp off 1 on some multi-section edges, 208
/// pitzDailySteady points); both flavours keep the computed value. If `n` is
/// smaller than the number of segments — OpenFOAM cannot
/// place at least one division per section — the edge is meshed uniformly, as
/// OpenFOAM does (`lineDivide.C` line 128).
///
/// Within a segment of `m` cells and per-cell growth `g = expansion^(1/(m-1))`,
/// the cumulative position of the `p`-th interior node is
/// `blockFrac * (1 - g^p) / (1 - g^m)` offset by the segment start — the same
/// geometric-series partial sum used by the single-ratio [`graded_positions`].
fn graded_positions_multi(
    n: usize,
    segments: &[GradingSegment],
    flavour: ProjectionFlavour,
) -> Vec<f64> {
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
        // Dev: `expRatio == 1.0`; v2506: `equal(expRatio, 1)`, i.e.
        // `mag(expRatio - 1) <= VSMALL` (1e-300).
        let unit_ratio = match flavour {
            ProjectionFlavour::Dev => expansion == 1.0,
            ProjectionFlavour::Esi => (expansion - 1.0).abs() <= 1.0e-300,
        };
        if unit_ratio {
            for (offset, slot) in s[secn_start..secn_end].iter_mut().enumerate() {
                // `p` is the node index within the section (1..=secn_div).
                let p = (offset + 1) as f64;
                *slot = sec_start + block_frac * p / secn_div as f64;
            }
        } else {
            // Per-cell growth factor; OpenFOAM `calcGexp` returns 0 for a single
            // cell, which makes the single-node formula collapse to blockFrac.
            let exp_fact = if secn_div > 1 {
                expansion.r_powf(1.0 / (secn_div as f64 - 1.0))
            } else {
                0.0
            };
            // ~~`pow(expFact, int)` in C++ is `pow(double, double)`, not repeated
            // multiplication: use the real power, as upstream.~~ **CORRECTED
            // 2026-11-11 (gh:#1364):** true of v2506 only. In OpenFOAM-dev
            // `pow(Scalar, int32_t)` is `IntegerPow` (repeated squaring,
            // `Scalar.H`), so the dev flavour uses [`integer_pow_dev`] and the
            // v2506 flavour the real power ([`line_divide_pow`]).
            let denom = 1.0 - line_divide_pow(exp_fact, secn_div, flavour);
            for (offset, slot) in s[secn_start..secn_end].iter_mut().enumerate() {
                // `offset + 1` is the node index within the section (1..=secn_div).
                let numer = 1.0 - line_divide_pow(exp_fact, offset + 1, flavour);
                *slot = sec_start + block_frac * numer / denom;
            }
        }
        sec_start = s[secn_end - 1];
        secn_start = secn_end;
    }
    // `divisions_[0] = 0` as upstream; the last node keeps its computed value
    // (no clamp, see the doc comment).
    s[0] = 0.0;
    s
}

#[cfg(test)]
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

/// Per-edge node distributions (the `[0, 1]` `lambdaDivisions` of each of the
/// 12 hex edges), laid out in OpenFOAM edge order.
///
/// Entry `e` has `n_dir + 1` monotone positions on `[0, 1]`, where the
/// direction is `e / 4` (edges `0..3` = x with `nx + 1` entries, `4..7` = y,
/// `8..11` = z). Mirrors the `edgeWeights[12]` array assembled by
/// `blockDescriptor::edgesPointsWeights`
/// (`blockDescriptor/blockDescriptorEdges.C:113-143`), whose entries are the
/// straight-edge `lineDivide::lambdaDivisions()` for each edge.
type EdgeLambdas = [Vec<f64>; 12];

#[cfg(test)]
/// Build the 12 per-edge `[0, 1]` node-position arrays for a block, from its 12
/// per-edge gradings and its `(nx, ny, nz)` cell counts.
///
/// Edge → direction/count follows OpenFOAM `edgesPointsWeights`
/// (`blockDescriptorEdges.C:121-141`): edges `0,1,2,3` are subdivided into `nx`
/// cells, `4,5,6,7` into `ny`, `8,9,10,11` into `nz`. Each edge's array is that
/// edge's own [`Grading::node_positions`] — a single ratio or a multi-segment
/// distribution.
fn edge_lambdas(edge_grading: &[Grading; 12], cells: [usize; 3]) -> EdgeLambdas {
    let [nx, ny, nz] = cells;
    std::array::from_fn(|e| {
        let n = match e / 4 {
            0 => nx,
            1 => ny,
            _ => nz,
        };
        edge_grading[e].node_positions(n)
    })
}

#[cfg(test)]
/// One interior node position `[m]` of a per-edge-graded hex block, blended
/// from all 12 edge distributions.
///
/// Faithful port of the straight-edge branch of OpenFOAM's
/// `Foam::block::createPoints` (`blocks/block/blockCreate.C:45-183`, the
/// `nCurvedEdges == 0` path). `corners` are the 8 scaled block corners in hex
/// order (`corners[0] = p000 … corners[6] = p111`, `corners[7] = p011`, matching
/// [`HEX_CORNER`]); `el` are the [`EdgeLambdas`]; `(i, j, k)` index the node grid
/// (`0..=nx`, `0..=ny`, `0..=nz`).
///
/// The node is the mean of three directional estimates. Each estimate is a
/// convex blend (weights normalised to sum to 1, `blockCreate.C:97-127`) of the
/// four straight edges running in that direction, evaluated at the edge's local
/// parameter (`blockCreate.C:131-152`). For a block whose four edges per
/// direction agree this reduces *exactly* to trilinear interpolation, so it is
/// a strict generalisation of [`trilinear`]. All positions are in metres `[m]`
/// (the corners are already scaled by `convertToMeters`).
fn edge_blended_node(
    corners: &[Vector3; 8],
    el: &EdgeLambdas,
    i: usize,
    j: usize,
    k: usize,
) -> Vector3 {
    edge_blended_node_with(corners, el, None, i, j, k)
}

/// [`edge_blended_node`] plus, when `curved` holds the 12 edges' actual point
/// arrays (`p[e][i|j|k]`, `blockDescriptor::edgesPointsWeights`), upstream's
/// curved-edge correction (`blockCreate.C`, "Apply curved-edge correction"):
/// each straight-edge estimate is shifted by `w * (p_edge - edge_straight)`.
/// `None` is the straight-edge branch.
fn edge_blended_node_with(
    corners: &[Vector3; 8],
    el: &EdgeLambdas,
    curved: Option<&[Vec<Vector3>; 12]>,
    i: usize,
    j: usize,
    k: usize,
) -> Vector3 {
    // Corner aliases (blockCreate.C:52-60).
    let p000 = corners[0];
    let p100 = corners[1];
    let p110 = corners[2];
    let p010 = corners[3];
    let p001 = corners[4];
    let p101 = corners[5];
    let p111 = corners[6];
    let p011 = corners[7];

    // Edge weights (blockCreate.C macros w0..w11): x-edges read at i, y at j,
    // z at k.
    let w0 = el[0][i];
    let w1 = el[1][i];
    let w2 = el[2][i];
    let w3 = el[3][i];
    let w4 = el[4][j];
    let w5 = el[5][j];
    let w6 = el[6][j];
    let w7 = el[7][j];
    let w8 = el[8][k];
    let w9 = el[9][k];
    let w10 = el[10][k];
    let w11 = el[11][k];

    // x-direction blend weights (blockCreate.C:92-101).
    let mut wx1 = (1.0 - w0) * (1.0 - w4) * (1.0 - w8) + w0 * (1.0 - w5) * (1.0 - w9);
    let mut wx2 = (1.0 - w1) * w4 * (1.0 - w11) + w1 * w5 * (1.0 - w10);
    let mut wx3 = (1.0 - w2) * w7 * w11 + w2 * w6 * w10;
    let mut wx4 = (1.0 - w3) * (1.0 - w7) * w8 + w3 * (1.0 - w6) * w9;
    let sum_wx = wx1 + wx2 + wx3 + wx4;
    wx1 /= sum_wx;
    wx2 /= sum_wx;
    wx3 /= sum_wx;
    wx4 /= sum_wx;

    // y-direction blend weights (blockCreate.C:105-114).
    let mut wy1 = (1.0 - w4) * (1.0 - w0) * (1.0 - w8) + w4 * (1.0 - w1) * (1.0 - w11);
    let mut wy2 = (1.0 - w5) * w0 * (1.0 - w9) + w5 * w1 * (1.0 - w10);
    let mut wy3 = (1.0 - w6) * w3 * w9 + w6 * w2 * w10;
    let mut wy4 = (1.0 - w7) * (1.0 - w3) * w8 + w7 * (1.0 - w2) * w11;
    let sum_wy = wy1 + wy2 + wy3 + wy4;
    wy1 /= sum_wy;
    wy2 /= sum_wy;
    wy3 /= sum_wy;
    wy4 /= sum_wy;

    // z-direction blend weights (blockCreate.C:118-127).
    let mut wz1 = (1.0 - w8) * (1.0 - w0) * (1.0 - w4) + w8 * (1.0 - w3) * (1.0 - w7);
    let mut wz2 = (1.0 - w9) * w0 * (1.0 - w5) + w9 * w3 * (1.0 - w6);
    let mut wz3 = (1.0 - w10) * w1 * w5 + w10 * w2 * w6;
    let mut wz4 = (1.0 - w11) * (1.0 - w1) * w4 + w11 * (1.0 - w2) * w7;
    let sum_wz = wz1 + wz2 + wz3 + wz4;
    wz1 /= sum_wz;
    wz2 /= sum_wz;
    wz3 /= sum_wz;
    wz4 /= sum_wz;

    // Points on the 12 straight edges (blockCreate.C:131-144).
    let edgex1 = p000 + (p100 - p000) * w0;
    let edgex2 = p010 + (p110 - p010) * w1;
    let edgex3 = p011 + (p111 - p011) * w2;
    let edgex4 = p001 + (p101 - p001) * w3;

    let edgey1 = p000 + (p010 - p000) * w4;
    let edgey2 = p100 + (p110 - p100) * w5;
    let edgey3 = p101 + (p111 - p101) * w6;
    let edgey4 = p001 + (p011 - p001) * w7;

    let edgez1 = p000 + (p001 - p000) * w8;
    let edgez2 = p100 + (p101 - p100) * w9;
    let edgez3 = p110 + (p111 - p110) * w10;
    let edgez4 = p010 + (p011 - p010) * w11;

    // Mean of the three directional estimates (blockCreate.C:147-152).
    let mut point = (edgex1 * wx1
        + edgex2 * wx2
        + edgex3 * wx3
        + edgex4 * wx4
        + edgey1 * wy1
        + edgey2 * wy2
        + edgey3 * wy3
        + edgey4 * wy4
        + edgez1 * wz1
        + edgez2 * wz2
        + edgez3 * wz3
        + edgez4 * wz4)
        / 3.0;

    // Curved-edge correction vectors, summed in upstream's order.
    if let Some(p) = curved {
        let corx1 = (p[0][i] - edgex1) * wx1;
        let corx2 = (p[1][i] - edgex2) * wx2;
        let corx3 = (p[2][i] - edgex3) * wx3;
        let corx4 = (p[3][i] - edgex4) * wx4;

        let cory1 = (p[4][j] - edgey1) * wy1;
        let cory2 = (p[5][j] - edgey2) * wy2;
        let cory3 = (p[6][j] - edgey3) * wy3;
        let cory4 = (p[7][j] - edgey4) * wy4;

        let corz1 = (p[8][k] - edgez1) * wz1;
        let corz2 = (p[9][k] - edgez2) * wz2;
        let corz3 = (p[10][k] - edgez3) * wz3;
        let corz4 = (p[11][k] - edgez4) * wz4;

        point += corx1
            + corx2
            + corx3
            + corx4
            + cory1
            + cory2
            + cory3
            + cory4
            + corz1
            + corz2
            + corz3
            + corz4;
    }
    point
}

// ───────────────────────────────────────────────────────────────────────────
//  Build
// ───────────────────────────────────────────────────────────────────────────

impl BlockMeshDict {
    /// Build the mesh the way upstream `blockMesh` does (gh:#1464): every
    /// block is generated by the 12-edge blend (`block::createPoints`), points
    /// are merged **topologically** across the block faces the block topology
    /// shares (`blockMesh::calcMergeInfo`) and numbered by first appearance,
    /// cells are re-labelled by `cellShape(hex, labels, true)` (collapsing
    /// degenerate hexes to prisms, wedges, pyramids and tets), and the faces,
    /// owners and neighbours are numbered by `polyMesh::setTopology`:
    /// internal faces cell by cell in increasing neighbour order, then each
    /// patch in dictionary order, then the unnamed faces in `defaultPatch`.
    ///
    /// The result is checked as `blockMesh::check` does (curved edges that
    /// match no block edge or repeat another, patch faces that match no block
    /// face or point inwards) and, unless `checkBlockFaceOrientation no`, each
    /// block's faces must point away from its centre.
    ///
    /// # Errors
    /// [`MeshError::Construction`] for any of upstream's fatal errors: bad
    /// vertex labels, inside-out blocks, inconsistent gradings between
    /// neighbouring blocks, patch faces that belong to no block, or a failed
    /// check. [`MeshError::NotImplemented`] for `fastMerge yes`.
    pub fn build(&self) -> Result<PolyMesh, MeshError> {
        if self.fast_merge {
            return Err(MeshError::NotImplemented(
                "`fastMerge yes` (blockMeshMergeFast.C; gh:#1464 remaining item)".into(),
            ));
        }
        let scale = self.convert_to_meters;

        // Resolve the `edges` list against the raw vertices (upstream builds
        // edges from the unscaled vertices and scales the finished points).
        let curves: Vec<EdgeCurve> = self
            .edges
            .iter()
            .map(|e| e.resolve_with(&self.vertices, &self.geometry))
            .collect::<Result<_, _>>()?;

        let mut grids: Vec<BlockGrid> = Vec::with_capacity(self.blocks.len());
        let mut shapes: Vec<CellShape> = Vec::with_capacity(self.blocks.len());
        for (bi, block) in self.blocks.iter().enumerate() {
            for &vi in &block.vertices {
                if vi >= self.vertices.len() {
                    return Err(MeshError::Construction(format!(
                        "block references vertex {vi} but only {} vertices defined",
                        self.vertices.len()
                    )));
                }
            }
            if block.cells.iter().any(|&n| n == 0) {
                return Err(MeshError::Construction(
                    "block has a zero cell count".to_string(),
                ));
            }
            let shape = CellShape {
                model: CellModel::Hex,
                labels: block.vertices.to_vec(),
            };
            if self.check_block_face_orientation {
                self.check_block_orientation(bi, &shape)?;
            }
            grids.push(self.block_grid(block, &curves)?);
            shapes.push(shape);
        }

        // Block topology: the blocks as hex cells over the dictionary
        // vertices, with the `boundary` faces as patches.
        let names: Vec<String> = self.patches.iter().map(|p| p.name.clone()).collect();
        let topo_patch_faces: Vec<Vec<Face>> = self
            .patches
            .iter()
            .map(|p| p.faces.iter().map(|q| q.to_vec()).collect())
            .collect();
        let topo = mesh_from_shapes(self.vertices.len(), &shapes, &topo_patch_faces, &names)?;
        self.check(&topo)?;

        let merge = calc_merge_info(
            &grids,
            &topo,
            &self.vertices,
            self.check_face_correspondence,
        )?;
        let points = create_points(&grids, &merge, scale);
        let cells = create_cells(&grids, &merge)?;

        // Patches of the topology, with the default patch last when non-empty.
        let n_default = topo.faces.len() - topo.default_start;
        let mut patch_names = names.clone();
        let mut patch_kinds: Vec<PatchKind> = self.patches.iter().map(|p| p.kind).collect();
        let mut ranges: Vec<(usize, usize)> = topo
            .patch_starts
            .iter()
            .zip(&topo.patch_sizes)
            .map(|(&s, &n)| (s, n))
            .collect();
        // A patch the dictionary itself declares under the default patch's name
        // takes the unassigned faces (and keeps its declared type), as upstream's
        // `polyMesh` shape-mesh constructor does (gh:#1363 row 7: the TJunction
        // tutorials declare `defaultFaces { type wall; faces (); }`; before this
        // the port wrote that patch empty AND a second `defaultFaces`).
        let declared_default = if n_default > 0 {
            patch_names.iter().position(|n| *n == self.default_patch_name)
        } else {
            None
        };
        if n_default > 0 && declared_default.is_none() {
            patch_names.push(self.default_patch_name.clone());
            patch_kinds.push(self.default_patch_kind);
            ranges.push((topo.default_start, n_default));
        }
        let mut patch_faces: Vec<Vec<Face>> = Vec::with_capacity(ranges.len());
        for &(start, size) in &ranges {
            let faces = &topo.faces[start..start + size];
            let owners = &topo.owner[start..start + size];
            patch_faces.push(create_patch_faces(&grids, &shapes, &merge, faces, owners));
        }
        if let Some(k) = declared_default {
            let start = topo.default_start;
            let faces = &topo.faces[start..start + n_default];
            let owners = &topo.owner[start..start + n_default];
            let extra = create_patch_faces(&grids, &shapes, &merge, faces, owners);
            patch_faces[k].extend(extra);
        }

        let sm = mesh_from_shapes(merge.n_points, &cells, &patch_faces, &patch_names)?;
        if sm.default_start != sm.faces.len() {
            return Err(MeshError::Construction(format!(
                "{} cell faces belong to no patch (default patch `{}` already has faces)",
                sm.faces.len() - sm.default_start,
                self.default_patch_name
            )));
        }

        let n_internal_faces = sm.n_internal();
        let faces: Vec<MeshFace> = sm
            .faces
            .iter()
            .enumerate()
            .map(|(fi, f)| MeshFace {
                verts: f.clone(),
                owner: sm.owner[fi],
                neighbour: (fi < n_internal_faces).then(|| sm.neighbour[fi]),
            })
            .collect();
        let patches: Vec<BoundaryPatch> = patch_names
            .iter()
            .zip(&patch_kinds)
            .enumerate()
            .map(|(pi, (name, &kind))| {
                BoundaryPatch::new(name.clone(), sm.patch_starts[pi], sm.patch_sizes[pi], kind)
            })
            .collect();

        // Cell zones: block cells are contiguous, in block order.
        let mut cell_zones: Vec<(String, Vec<usize>)> = Vec::new();
        let mut cell_count = 0usize;
        for (block, grid) in self.blocks.iter().zip(&grids) {
            if let Some(zone) = &block.zone {
                let range = cell_count..cell_count + grid.n_cells();
                match cell_zones.iter_mut().find(|(n, _)| n == zone) {
                    Some((_, v)) => v.extend(range),
                    None => cell_zones.push((zone.clone(), range.collect())),
                }
            }
            cell_count += grid.n_cells();
        }

        Ok(PolyMesh {
            points,
            faces,
            n_internal_faces,
            n_cells: cells.len(),
            patches,
            cell_zones,
        })
    }

    /// `block::createPoints()` for one block: raw (unscaled) points, the 12
    /// edges' weights blended by `blockCreate.C:45-183`, with the curved-edge
    /// correction when any edge is listed. The eight block vertices are set
    /// exactly (the `vertex(i, j, k)` skip).
    fn block_grid(&self, block: &Block, curves: &[EdgeCurve]) -> Result<BlockGrid, MeshError> {
        let [nx, ny, nz] = block.cells;
        let (el, ep) = self.edge_data(block, curves);
        let raw: [Vector3; 8] = std::array::from_fn(|c| self.vertices[block.vertices[c]]);
        let mut points = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
        for k in 0..=nz {
            for j in 0..=ny {
                for i in 0..=nx {
                    let cx = i == nx;
                    let cy = j == ny;
                    let cz = k == nz;
                    let is_corner = (i == 0 || cx) && (j == 0 || cy) && (k == 0 || cz);
                    let p = if is_corner {
                        let base = if !cx && !cy {
                            0
                        } else if cx && !cy {
                            1
                        } else if cx && cy {
                            2
                        } else {
                            3
                        };
                        raw[base + if cz { 4 } else { 0 }]
                    } else {
                        edge_blended_node_with(&raw, &el, ep.as_ref(), i, j, k)
                    };
                    points.push(p);
                }
            }
        }
        let curved = find_curved_faces(&block.vertices, &self.faces);
        if curved.iter().any(Option::is_some) {
            apply_curved_faces(
                &self.geometry,
                &self.faces,
                &curved,
                &block.vertices,
                block.cells,
                &el,
                &mut points,
            )?;
        }
        Ok(BlockGrid {
            n: block.cells,
            points,
        })
    }

    /// `blockDescriptor::check`: each face of the block must point away from
    /// the block centre (some other face centre lies behind it).
    fn check_block_orientation(&self, bi: usize, shape: &CellShape) -> Result<(), MeshError> {
        let faces = shape.faces();
        let mut outward = 0usize;
        let mut correct = vec![true; faces.len()];
        for (i, f) in faces.iter().enumerate() {
            let (centre, area) = face_centre_and_area(&self.vertices, f);
            if area.mag() > 1.0e-15 {
                let is_out = faces.iter().enumerate().any(|(j, g)| {
                    j != i && {
                        let (cj, _) = face_centre_and_area(&self.vertices, g);
                        let d = centre - cj;
                        d.x * area.x + d.y * area.y + d.z * area.z > 0.0
                    }
                });
                if is_out {
                    outward += 1;
                } else {
                    correct[i] = false;
                }
            } else {
                outward += 1;
            }
        }
        if outward == 0 {
            return Err(MeshError::Construction(format!("block {bi} is inside-out")));
        }
        if outward != faces.len() {
            let bad: Vec<_> = faces
                .iter()
                .zip(&correct)
                .filter(|(_, &ok)| !ok)
                .map(|(f, _)| format!("{f:?}"))
                .collect();
            return Err(MeshError::Construction(format!(
                "block {bi} has inward-pointing faces {}",
                bad.join(" ")
            )));
        }
        Ok(())
    }

    /// `blockMesh::check`: curved edges that repeat or match no block edge,
    /// patch faces that match no block face or point inwards.
    fn check(&self, topo: &ShapeMesh) -> Result<(), MeshError> {
        let mut msg = String::new();
        for (cei, e) in self.edges.iter().enumerate() {
            if let Some(cej) = self.edges[cei + 1..]
                .iter()
                .position(|o| e.compare(o.start, o.end) != 0)
            {
                msg.push_str(&format!(
                    "    Curved edge #{cei} is a duplicate of curved edge #{}\n",
                    cei + 1 + cej
                ));
            }
        }
        for (cei, e) in self.edges.iter().enumerate() {
            let found = self.blocks.iter().any(|b| {
                HEX_EDGE_ENDS
                    .iter()
                    .any(|&(s, t)| e.compare(b.vertices[s], b.vertices[t]) != 0)
            });
            if !found {
                msg.push_str(&format!(
                    "    Curved edge #{cei} does not correspond to a block edge\n"
                ));
            }
        }
        // Patch-face consistency. Upstream iterates `bm.boundary()`, whose
        // faces are the mesh's own faces (each cell's outward face), not the
        // dictionary's quads, so a dictionary quad wound the wrong way is
        // accepted and the "points inwards" test only fires for a mesh face
        // that is not its cell's outward face. Ported as upstream does it.
        for pi in 0..topo.patch_starts.len() {
            let (start, size) = (topo.patch_starts[pi], topo.patch_sizes[pi]);
            for fi in 0..size {
                let patch_face = &topo.faces[start + fi];
                let mut ok = false;
                for cell in &topo.cells {
                    for &cf in cell {
                        let tf = &topo.faces[cf];
                        if faces_equal(patch_face, tf) {
                            ok = true;
                            let a = face_centre_and_area(&self.vertices, patch_face).1;
                            let b = face_centre_and_area(&self.vertices, tf).1;
                            if a.x * b.x + a.y * b.y + a.z * b.z < 0.0 {
                                msg.push_str(&format!(
                                    "    Face #{fi} {patch_face:?} of patch #{pi} {} points inwards\n",
                                    self.patches[pi].name
                                ));
                            }
                        }
                    }
                }
                if !ok {
                    msg.push_str(&format!(
                        "    Face #{fi} {patch_face:?} of patch #{pi} {} does not match any block faces\n",
                        self.patches[pi].name
                    ));
                }
            }
        }
        if msg.is_empty() {
            Ok(())
        } else {
            Err(MeshError::Construction(format!(
                "blockMesh topology check failed:\n{msg}"
            )))
        }
    }

    /// The 12 edges' weights, and their point arrays when at least one edge
    /// is listed in `edges` (upstream `blockDescriptor::edgesPointsWeights`
    /// with `nCurvedEdges > 0`); the points are `None` when all twelve are
    /// plain straight edges (the `nCurvedEdges == 0` branch of
    /// `block::createPoints`).
    ///
    /// Per edge, the first listed edge that matches the block's vertex pair
    /// wins; a reversed match divides the edge with the inverted grading and
    /// reverses the result (`weights = 1 - d`), exactly as upstream.
    fn edge_data(
        &self,
        block: &Block,
        curves: &[EdgeCurve],
    ) -> (EdgeLambdas, Option<[Vec<Vector3>; 12]>) {
        let raw: [Vector3; 8] = std::array::from_fn(|c| self.vertices[block.vertices[c]]);
        let mut n_curved = 0usize;
        let mut weights: Vec<Vec<f64>> = Vec::with_capacity(12);
        let mut points: Vec<Vec<Vector3>> = Vec::with_capacity(12);
        for (e, &(ls, le)) in HEX_EDGE_ENDS.iter().enumerate() {
            let n = block.cells[e / 4];
            let grading = match &block.edge_grading {
                Some(eg) => &eg[e],
                None => &block.grading[e / 4],
            };
            let (gs, ge) = (block.vertices[ls], block.vertices[le]);
            let hit = self
                .edges
                .iter()
                .enumerate()
                .find_map(|(ci, edge)| match edge.compare(gs, ge) {
                    0 => None,
                    cmp => Some((ci, cmp)),
                });
            match hit {
                Some((ci, cmp)) if cmp > 0 => {
                    let w = line_divide_weights(n, grading, self.geometry.flavour);
                    points.push(curves[ci].positions(&w));
                    weights.push(w);
                    n_curved += 1;
                }
                Some((ci, _)) => {
                    let d = line_divide_weights(n, &grading.inv(), self.geometry.flavour);
                    let p = curves[ci].positions(&d);
                    let pn = p.len() - 1;
                    points.push((0..=pn).map(|pi| p[pn - pi]).collect());
                    weights.push((0..=pn).map(|pi| 1.0 - d[pn - pi]).collect());
                    n_curved += 1;
                }
                None => {
                    let w = line_divide_weights(n, grading, self.geometry.flavour);
                    let (a, b) = (raw[ls], raw[le]);
                    if !self.edges.is_empty() {
                        points.push(w.iter().map(|&l| a + (b - a) * l).collect());
                    }
                    weights.push(w);
                }
            }
        }
        let el: EdgeLambdas = std::array::from_fn(|e| std::mem::take(&mut weights[e]));
        if n_curved == 0 {
            return (el, None);
        }
        let ep: [Vec<Vector3>; 12] = std::array::from_fn(|e| std::mem::take(&mut points[e]));
        (el, Some(ep))
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
    /// `convertToMeters`/`scale`/`units`, `geometry`, `vertices` (including
    /// `project`), `blocks`, `edges` (including `project`/`projectCurve`),
    /// `faces`, `defaultPatch`, `boundary`/`patches` and an empty
    /// `mergePatchPairs` entries, after expanding `$name` variables. See the module docs for
    /// deferred features.
    ///
    /// # Errors
    /// [`MeshError::DictParse`] on malformed syntax (or more than one of
    /// `convertToMeters`/`scale`/`units`, or a `defaultPatch` without `type`);
    /// [`MeshError::NotImplemented`] for an unsupported block shape (only
    /// `hex`), a macro that is not expanded, or a keyword listed in the module
    /// docs (a surface type that is not ported, non-empty
    /// `mergePatchPairs`, ...). All grading forms —
    /// `simpleGrading`, multi-grading, and full per-edge `edgeGrading` — parse.
    pub fn parse(text: &str) -> Result<Self, MeshError> {
        Self::parse_with_flavour(text, ProjectionFlavour::Dev)
    }

    /// As [`parse`](Self::parse), following v2506 instead of dev where the two
    /// differ in projection (`ProjectionFlavour`, gh:#1462): used only to compare
    /// with a v2506 `blockMesh` run.
    ///
    /// # Errors
    /// As [`parse`](Self::parse).
    pub fn parse_with_flavour(text: &str, flavour: ProjectionFlavour) -> Result<Self, MeshError> {
        let tokens = expand_variables(tokenize(text))?;
        let mut cur = Cursor::new(&tokens);

        // Dictionary macros are not expanded (gh:#1463): refuse rather than
        // read `$x` as a literal and mesh something else.
        if let Some(t) = tokens
            .iter()
            .find(|t| t.starts_with('$') || t.starts_with('#'))
        {
            return Err(MeshError::NotImplemented(format!(
                "dictionary macro `{t}` ($var / #calc / #include) is not expanded"
            )));
        }

        let mut convert_to_meters = 1.0;
        let mut n_scale_keywords = 0usize;
        let mut vertices = Vec::new();
        let mut blocks = Vec::new();
        let mut raw_patches: Vec<RawPatch> = Vec::new();
        let mut edges = Vec::new();
        let mut default_patch_name = "defaultFaces".to_string();
        let mut default_patch_kind = PatchKind::Empty;
        let mut fast_merge = false;
        let mut check_face_correspondence = true;
        let mut check_block_face_orientation = true;
        // gh:#1364: the flavour must reach a dict WITHOUT a `geometry` section too
        // (it was dropped: `Geometry::default()` is the dev flavour), or the ESI
        // `lineDivide` pow and spline forms were never selected for such dicts.
        let mut geometry = Arc::new(Geometry { surfaces: Vec::new(), flavour });
        let mut faces: Vec<BlockFace> = Vec::new();
        let mut vertex_projections: Vec<Option<Vec<String>>> = Vec::new();

        while let Some(tok) = cur.peek() {
            match tok {
                "convertToMeters" | "scale" => {
                    cur.next();
                    convert_to_meters = cur.parse_f64()?;
                    cur.expect(";")?;
                    n_scale_keywords += 1;
                }
                "units" => {
                    cur.next();
                    let mut expr = String::new();
                    loop {
                        match cur.next() {
                            Some(";") => break,
                            Some(t) => expr.push_str(t),
                            None => {
                                return Err(MeshError::DictParse("unterminated `units`".into()))
                            }
                        }
                    }
                    convert_to_meters = length_units_to_metres(&expr)?;
                    n_scale_keywords += 1;
                }
                "vertices" => {
                    cur.next();
                    (vertices, vertex_projections) = parse_vertices(&mut cur)?;
                    cur.expect(";")?;
                }
                "blocks" => {
                    cur.next();
                    blocks = parse_blocks(&mut cur)?;
                    cur.expect(";")?;
                }
                "edges" => {
                    cur.next();
                    edges = parse_edges(&mut cur)?;
                    cur.expect(";")?;
                }
                "defaultPatch" => {
                    cur.next();
                    let (name, kind) = parse_default_patch(&mut cur)?;
                    if let Some(n) = name {
                        default_patch_name = n;
                    }
                    default_patch_kind = kind;
                }
                "namedVertices" | "namedBlocks" => {
                    // `namedVertices { a 0; b 1; }`: explicit name table.
                    cur.next();
                    cur.expect("{")?;
                    while !cur.accept("}") {
                        let name = cur
                            .next()
                            .ok_or_else(|| MeshError::DictParse(format!("unterminated `{tok}`")))?
                            .to_string();
                        let idx = cur.parse_usize()?;
                        cur.expect(";")?;
                        if tok == "namedVertices" {
                            cur.vertex_names.insert(name, idx);
                        } else {
                            cur.block_names.insert(name, idx);
                        }
                    }
                }
                "geometry" => {
                    cur.next();
                    let entries = read_dict_body(cur.tokens, &mut cur.pos)?;
                    geometry = Arc::new(Geometry::from_entries(&entries, flavour)?);
                }
                "faces" => {
                    cur.next();
                    faces = parse_block_faces(&mut cur)?;
                    cur.expect(";")?;
                }
                "boundary" | "patches" => {
                    cur.next();
                    raw_patches = parse_boundary(&mut cur)?;
                    cur.expect(";")?;
                }
                "mergePatchPairs" => {
                    cur.next();
                    cur.expect("(")?;
                    if !cur.accept(")") {
                        return Err(MeshError::NotImplemented(
                            "non-empty `mergePatchPairs` (gh:#1464)".into(),
                        ));
                    }
                    cur.expect(";")?;
                }
                "fastMerge" | "checkFaceCorrespondence" | "checkBlockFaceOrientation"
                | "verbose" => {
                    cur.next();
                    let v = match cur.next() {
                        Some("yes" | "on" | "true" | "y" | "t") => true,
                        Some("no" | "off" | "false" | "n" | "f" | "none") => false,
                        other => {
                            return Err(MeshError::DictParse(format!(
                                "`{tok}` expects a switch, found {other:?}"
                            )))
                        }
                    };
                    cur.expect(";")?;
                    match tok {
                        "fastMerge" => fast_merge = v,
                        "checkFaceCorrespondence" => check_face_correspondence = v,
                        "checkBlockFaceOrientation" => check_block_face_orientation = v,
                        _ => {}
                    }
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

        // `project` vertices (blockVertices::projectVertex): the surfaces may be
        // declared after the vertices, so project once everything is read.
        for (v, proj) in vertices.iter_mut().zip(&vertex_projections) {
            if let Some(names) = proj {
                let ids = geometry.surface_ids(names)?;
                *v = geometry.project_vertex(&ids, *v);
            }
        }
        for f in &faces {
            if let BlockFaceKind::Project(name) = &f.kind {
                geometry.surface_ids(std::slice::from_ref(name))?;
            }
        }

        let patches = resolve_patch_faces(raw_patches, &blocks, vertices.len())?;

        if n_scale_keywords > 1 {
            return Err(MeshError::DictParse(
                "more than one of keywords 'convertToMeters', 'scale' and 'units' defined".into(),
            ));
        }

        Ok(BlockMeshDict {
            convert_to_meters,
            vertices,
            blocks,
            patches,
            edges,
            default_patch_name,
            default_patch_kind,
            fast_merge,
            check_face_correspondence,
            check_block_face_orientation,
            geometry,
            faces,
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

/// Expand `$name` references to earlier top-level `name value;` entries
/// (the dictionary-macro subset the OpenFOAM blockMesh tutorials use, e.g.
/// `pitzDailySteady`'s `$posY`; gh:#1463).
///
/// A top-level entry is `key tokens... ;` (parentheses balanced) or
/// `key { ... }`. Its value tokens, themselves expanded, are recorded under
/// `key`; a later `$key` token, anywhere, is replaced by them. Upstream looks
/// the name up in the enclosing dictionary the same way, so a use before the
/// definition is not expanded. Anything left starting with `$` or `#`
/// (`${x}`, `$:a.b`, `#calc`, `#include`) is reported by [`BlockMeshDict::parse`].
fn expand_variables(tokens: Vec<String>) -> Result<Vec<String>, MeshError> {
    let mut vars: HashMap<String, Vec<String>> = HashMap::new();
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let sub = |t: &String, vars: &HashMap<String, Vec<String>>, out: &mut Vec<String>| {
        match t.strip_prefix('$').and_then(|n| vars.get(n)) {
            Some(v) => out.extend(v.iter().cloned()),
            None => out.push(t.clone()),
        }
    };
    let mut i = 0usize;
    while i < tokens.len() {
        let key = tokens[i].clone();
        if key.starts_with('$') || key.starts_with('#') || key == ";" {
            out.push(key);
            i += 1;
            continue;
        }
        out.push(key.clone());
        i += 1;
        let mut value: Vec<String> = Vec::new();
        if tokens.get(i).map(|t| t.as_str()) == Some("{") {
            let mut depth = 0i32;
            while i < tokens.len() {
                let t = &tokens[i];
                if t == "{" {
                    depth += 1;
                } else if t == "}" {
                    depth -= 1;
                }
                sub(t, &vars, &mut out);
                i += 1;
                if depth == 0 {
                    break;
                }
            }
            continue;
        }
        let mut depth = 0i32;
        while i < tokens.len() {
            let t = &tokens[i];
            if t == "(" {
                depth += 1;
            } else if t == ")" {
                depth -= 1;
            } else if t == ";" && depth <= 0 {
                break;
            }
            sub(t, &vars, &mut value);
            i += 1;
        }
        out.extend(value.iter().cloned());
        if i < tokens.len() {
            out.push(";".into());
            i += 1;
        }
        vars.insert(key, value);
    }
    Ok(out)
}

/// A forward-only token cursor with small parsing helpers.
struct Cursor<'a> {
    tokens: &'a [String],
    pos: usize,
    /// `namedVertices`: name to vertex index (`name v0 (x y z)`).
    vertex_names: HashMap<String, usize>,
    /// `namedBlocks`: name to block index (`name b0 hex (...)`).
    block_names: HashMap<String, usize>,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [String]) -> Self {
        Self {
            tokens,
            pos: 0,
            vertex_names: HashMap::new(),
            block_names: HashMap::new(),
        }
    }
    /// A label: an integer, or a name from `namedVertices`
    /// (`blockMeshTools::read<label>`). With `blocks_too`, `namedBlocks` names
    /// resolve as well (the `varDict` of the patch lists).
    fn parse_label(&mut self, blocks_too: bool) -> Result<usize, MeshError> {
        let t = self
            .next()
            .ok_or_else(|| MeshError::DictParse("expected a label, found end".into()))?;
        if let Ok(v) = t.parse::<usize>() {
            return Ok(v);
        }
        if let Some(&v) = self.vertex_names.get(t) {
            return Ok(v);
        }
        if blocks_too {
            if let Some(&v) = self.block_names.get(t) {
                return Ok(v);
            }
        }
        Err(MeshError::DictParse(format!(
            "undefined variable `{t}` (valid names: {:?})",
            self.vertex_names.keys().collect::<Vec<_>>()
        )))
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

/// Metres per unit for a `units [..]` expression of dimension length
/// (`blockMeshTopology.C`: `unitSet units(dimensions::length); units.read(..);
/// scaleFactor_ = units.toStandard(1)`).
///
/// The multipliers are those of the length units in OpenFOAM-dev
/// `etc/configDict` (`units` table, set `SI`): `m`, `um` 1e-6, `mm` 1e-3,
/// `cm` 1e-2, `km` 1e3, `ft` 0.3048, `in` 25.4 mm, `yd` 3 ft, `mi` 1760 yd.
/// A single unit name is supported; a compound expression is reported as
/// not implemented rather than guessed.
///
/// # Errors
/// [`MeshError::NotImplemented`] for any other expression.
fn length_units_to_metres(expr: &str) -> Result<f64, MeshError> {
    let inner = expr.trim().trim_start_matches('[').trim_end_matches(']').trim();
    let ft = 0.3048;
    let yd = 3.0 * ft;
    match inner {
        "m" => Ok(1.0),
        "um" => Ok(1e-6),
        "mm" => Ok(1e-3),
        "cm" => Ok(1e-2),
        "km" => Ok(1e3),
        "ft" => Ok(ft),
        "in" => Ok(25.4 * 1e-3),
        "yd" => Ok(yd),
        "mi" => Ok(1760.0 * yd),
        other => Err(MeshError::NotImplemented(format!(
            "`units [{other}]`: only a single length unit (m, um, mm, cm, km, in, ft, yd, mi) is supported"
        ))),
    }
}

/// Parse `defaultPatch { name n; type t; }` (the cursor is on `{`). `type` is
/// required, as upstream (`dictPtr->lookup<word>("type")`).
fn parse_default_patch(cur: &mut Cursor) -> Result<(Option<String>, PatchKind), MeshError> {
    cur.expect("{")?;
    let mut name = None;
    let mut kind = None;
    loop {
        match cur.next() {
            Some("}") => break,
            Some("name") => {
                name = Some(
                    cur.next()
                        .ok_or_else(|| MeshError::DictParse("expected a patch name".into()))?
                        .to_string(),
                );
                cur.expect(";")?;
            }
            Some("type") => {
                let t = cur
                    .next()
                    .ok_or_else(|| MeshError::DictParse("expected a patch type".into()))?;
                kind = Some(patch_kind_from_str(t));
                cur.expect(";")?;
            }
            Some(_) => {
                // Other patch-dictionary keys (e.g. `inGroups`) do not change the mesh.
                while let Some(t) = cur.next() {
                    if t == ";" {
                        break;
                    }
                }
            }
            None => return Err(MeshError::DictParse("unterminated defaultPatch".into())),
        }
    }
    let kind = kind.ok_or_else(|| {
        MeshError::DictParse("keyword type is undefined in dictionary defaultPatch".into())
    })?;
    Ok((name, kind))
}

/// Parse `( (x y z) )` into points.
fn parse_point_list(cur: &mut Cursor) -> Result<Vec<Vector3>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        out.push(parse_vector(cur)?);
    }
    Ok(out)
}

/// Parse `(x y z)`.
fn parse_vector(cur: &mut Cursor) -> Result<Vector3, MeshError> {
    cur.expect("(")?;
    let x = cur.parse_f64()?;
    let y = cur.parse_f64()?;
    let z = cur.parse_f64()?;
    cur.expect(")")?;
    Ok(Vector3::new(x, y, z))
}

/// Parse the `edges ( ... )` list (OpenFOAM `blockEdge::New`): `line`, `arc`
/// (through a point, or angle and axis), `polyLine`, `spline` and `BSpline`.
///
/// # Errors
/// ~~[`MeshError::NotImplemented`] for `project` / `projectCurve`~~ **CORRECTED
/// 2026-11-11 (gh:#1462)**: they parse to [`BlockEdgeKind::Project`] /
/// [`BlockEdgeKind::ProjectCurve`]; [`MeshError::DictParse`] for an unknown type.
fn parse_edges(cur: &mut Cursor) -> Result<Vec<BlockEdge>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        let ty = cur
            .next()
            .ok_or_else(|| MeshError::DictParse("expected an edge type, found end".into()))?;
        let start = cur.parse_label(false)?;
        let end = cur.parse_label(false)?;
        let kind = match ty {
            "line" => BlockEdgeKind::Line,
            "arc" => {
                if cur.peek() == Some("(") {
                    BlockEdgeKind::ArcThroughPoint(parse_vector(cur)?)
                } else {
                    let angle_deg = cur.parse_f64()?;
                    let axis = parse_vector(cur)?;
                    BlockEdgeKind::ArcAngleAxis { angle_deg, axis }
                }
            }
            "polyLine" => BlockEdgeKind::PolyLine(parse_point_list(cur)?),
            "spline" | "BSpline" => {
                let pts = parse_point_list(cur)?;
                // Optional start/end tangents: read and discarded upstream.
                if cur.peek() == Some("(") {
                    parse_vector(cur)?;
                    parse_vector(cur)?;
                }
                if ty == "spline" {
                    BlockEdgeKind::Spline(pts)
                } else {
                    BlockEdgeKind::BSpline(pts)
                }
            }
            "project" => BlockEdgeKind::Project(parse_word_list(cur)?),
            "projectCurve" => BlockEdgeKind::ProjectCurve(parse_word_list(cur)?),
            other => {
                return Err(MeshError::DictParse(format!("unknown blockEdge type `{other}`")));
            }
        };
        out.push(BlockEdge { start, end, kind });
    }
    Ok(out)
}

/// Parse `( (x y z) | project (x y z) (surface ...) ... )` into raw (unscaled)
/// points, plus each vertex's projection surface names (`blockVertex::New`).
#[allow(clippy::type_complexity)]
fn parse_vertices(
    cur: &mut Cursor,
) -> Result<(Vec<Vector3>, Vec<Option<Vec<String>>>), MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    let mut proj: Vec<Option<Vec<String>>> = Vec::new();
    while !cur.accept(")") {
        if cur.peek() == Some("name") {
            cur.next();
            let name = cur
                .next()
                .ok_or_else(|| MeshError::DictParse("expected a vertex name".into()))?
                .to_string();
            cur.vertex_names.insert(name, out.len());
        }
        let mut surfaces = None;
        let projected = cur.peek() == Some("project");
        if projected {
            cur.next();
        }
        cur.expect("(")?;
        let x = cur.parse_f64()?;
        let y = cur.parse_f64()?;
        let z = cur.parse_f64()?;
        cur.expect(")")?;
        if projected {
            surfaces = Some(parse_word_list(cur)?);
        }
        out.push(Vector3::new(x, y, z));
        proj.push(surfaces);
    }
    Ok((out, proj))
}

/// Parse `(word word ...)`.
fn parse_word_list(cur: &mut Cursor) -> Result<Vec<String>, MeshError> {
    cur.expect("(")?;
    let mut names = Vec::new();
    while !cur.accept(")") {
        names.push(
            cur.next()
                .ok_or_else(|| MeshError::DictParse("unterminated word list".into()))?
                .to_string(),
        );
    }
    Ok(names)
}

/// Parse the `faces ( project (v0 v1 v2 v3) surface  sweep (v0 v1 v2 v3) f ... )`
/// list (`blockFace::New`).
///
/// # Errors
/// [`MeshError::DictParse`] for an unknown face type, a face that is not four
/// vertices, or a sweep fraction outside `[0, 1]`.
fn parse_block_faces(cur: &mut Cursor) -> Result<Vec<BlockFace>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        let ty = cur
            .next()
            .ok_or_else(|| MeshError::DictParse("expected a face type, found end".into()))?;
        cur.expect("(")?;
        let mut labels = Vec::new();
        while !cur.accept(")") {
            labels.push(cur.parse_label(false)?);
        }
        let vertices: [usize; 4] = labels.try_into().map_err(|_| {
            MeshError::DictParse(format!("block face of type `{ty}` needs four vertices"))
        })?;
        let kind = match ty {
            "project" => BlockFaceKind::Project(
                cur.next()
                    .ok_or_else(|| MeshError::DictParse("expected a surface name".into()))?
                    .to_string(),
            ),
            "sweep" => {
                let f = cur.parse_f64()?;
                if !(0.0..=1.0).contains(&f) {
                    return Err(MeshError::DictParse(format!(
                        "Rotation fraction for face {vertices:?} must be between 0 and 1"
                    )));
                }
                BlockFaceKind::Sweep(f)
            }
            other => {
                return Err(MeshError::DictParse(format!("Unknown blockFace type {other}")))
            }
        };
        out.push(BlockFace { vertices, kind });
    }
    Ok(out)
}

/// Parse the `blocks ( hex (...) (...) simpleGrading (...) ... )` list.
fn parse_blocks(cur: &mut Cursor) -> Result<Vec<Block>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        // Optional `name <blockName>` (namedBlock), then the shape keyword;
        // only `hex` is supported.
        if cur.peek() == Some("name") {
            cur.next();
            let name = cur
                .next()
                .ok_or_else(|| MeshError::DictParse("expected a block name".into()))?
                .to_string();
            cur.block_names.insert(name, out.len());
        }
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
            *v = cur.parse_label(false)?;
        }
        cur.expect(")")?;

        // Optional zone name between the vertex list and the cell counts.
        let zone = if cur.peek() != Some("(") {
            cur.next().map(|z| z.to_string())
        } else {
            None
        };

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
        let (grading, edge_grading) = match grading_kw {
            "simpleGrading" => {
                // Three per-direction gradings, each a scalar ratio or a
                // parenthesised multi-segment list.
                cur.expect("(")?;
                let gx = parse_grading_direction(cur)?;
                let gy = parse_grading_direction(cur)?;
                let gz = parse_grading_direction(cur)?;
                cur.expect(")")?;
                ([gx, gy, gz], None)
            }
            "edgeGrading" => {
                let edges = parse_edge_grading(cur)?;
                collapse_edge_grading(edges)
            }
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
            edge_grading,
            zone,
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
/// (`blockDescriptorEdges.C:121-141`): edges `0..3` are the x-direction, `4..7`
/// the y-direction, `8..11` the z-direction. Each entry is a scalar expansion
/// ratio ([`Grading::Uniform`]) or a parenthesised multi-segment list
/// ([`Grading::Multi`]); a negative ratio is trapped as its inverse.
///
/// Returns the 12 edge gradings in OpenFOAM order; [`collapse_edge_grading`]
/// then decides whether they reduce to a per-direction `simpleGrading` or need
/// the full 12-edge blend of `edge_blended_node`.
///
/// # Errors
/// [`MeshError::DictParse`] if the list does not hold exactly 12 entries.
fn parse_edge_grading(cur: &mut Cursor) -> Result<[Grading; 12], MeshError> {
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
    let mut it = edges.into_iter();
    Ok(std::array::from_fn(|_| it.next().unwrap()))
}

/// Decide how a parsed 12-edge `edgeGrading` is realised.
///
/// If the four edges of every direction share the same [`Grading`], the block
/// collapses to the equivalent `simpleGrading` (`edge_grading = None`), which is
/// *exact* and takes the fast trilinear path. Otherwise the genuine per-edge
/// distribution is kept (`edge_grading = Some(edges)`) so
/// `edge_blended_node` blends interior nodes from all 12 edges; the returned
/// `[Grading; 3]` is then just a representative (each direction's first edge)
/// for inspection.
///
/// The reduction is exact: when the four edges of a direction agree, the 12-edge
/// blend of `edge_blended_node` is algebraically identical to trilinear
/// interpolation with that shared per-direction distribution (see the function
/// docs), so the two paths produce bit-comparable node positions.
fn collapse_edge_grading(edges: [Grading; 12]) -> ([Grading; 3], Option<[Grading; 12]>) {
    let uniform_per_dir = (0..3).all(|d| {
        let base = d * 4;
        (1..4).all(|e| edges[base + e] == edges[base])
    });
    let rep: [Grading; 3] = [edges[0].clone(), edges[4].clone(), edges[8].clone()];
    if uniform_per_dir {
        (rep, None)
    } else {
        (rep, Some(edges))
    }
}

/// Parse the `boundary ( name { type t; faces ( ... ); } ... )` list.
fn parse_boundary(cur: &mut Cursor) -> Result<Vec<RawPatch>, MeshError> {
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
        out.push(RawPatch { name, kind, faces });
    }
    Ok(out)
}

/// One entry of a patch's `faces` list: four vertex labels, or
/// `(block face)` naming a face of a block (`blockMesh::checkPatchLabels`).
#[derive(Debug, Clone, Copy)]
enum RawFace {
    Quad([usize; 4]),
    BlockFace { block: usize, face: usize },
}

/// A parsed patch before `(block face)` entries are resolved.
struct RawPatch {
    name: String,
    kind: PatchKind,
    faces: Vec<RawFace>,
}

/// Parse `( (i j k l) (block face) ... )`; vertex and block names resolve.
fn parse_face_list(cur: &mut Cursor) -> Result<Vec<RawFace>, MeshError> {
    cur.expect("(")?;
    let mut out = Vec::new();
    while !cur.accept(")") {
        cur.expect("(")?;
        let mut labels = Vec::new();
        while !cur.accept(")") {
            labels.push(cur.parse_label(true)?);
        }
        match labels.as_slice() {
            [a, b, c, d] => out.push(RawFace::Quad([*a, *b, *c, *d])),
            [block, face] => out.push(RawFace::BlockFace {
                block: *block,
                face: *face,
            }),
            other => {
                return Err(MeshError::DictParse(format!(
                    "a patch face has 4 vertices or is (block face); found {other:?}"
                )))
            }
        }
    }
    Ok(out)
}

/// Replace `(block face)` entries by the block's face
/// (`block.blockShape().faces()[face]`), range-checked as upstream.
fn resolve_patch_faces(
    raw: Vec<RawPatch>,
    blocks: &[Block],
    n_vertices: usize,
) -> Result<Vec<PatchDef>, MeshError> {
    let hex = CellModel::Hex.faces();
    let mut out = Vec::with_capacity(raw.len());
    for p in raw {
        let mut faces = Vec::with_capacity(p.faces.len());
        for (fi, f) in p.faces.iter().enumerate() {
            match *f {
                RawFace::Quad(q) => {
                    if let Some(&bad) = q.iter().find(|&&v| v >= n_vertices) {
                        return Err(MeshError::DictParse(format!(
                            "point label {bad} out of range 0..{} on patch {}, face {fi}",
                            n_vertices.saturating_sub(1),
                            p.name
                        )));
                    }
                    faces.push(q);
                }
                RawFace::BlockFace { block, face } => {
                    let Some(b) = blocks.get(block) else {
                        return Err(MeshError::DictParse(format!(
                            "block index {block} out of range for patch face on patch {}, face {fi} \
                             (number of blocks = {})",
                            p.name,
                            blocks.len()
                        )));
                    };
                    if face >= hex.len() {
                        return Err(MeshError::DictParse(format!(
                            "block face index {face} out of range (6 block faces) on patch {}, \
                             face {fi}",
                            p.name
                        )));
                    }
                    faces.push(std::array::from_fn(|k| b.vertices[hex[face][k]]));
                }
            }
        }
        out.push(PatchDef {
            name: p.name,
            kind: p.kind,
            faces,
        });
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

    /// `from_foam_poly_mesh` inverts `to_foam_poly_mesh` exactly (gh:#1202).
    #[test]
    fn foam_poly_mesh_round_trip_is_exact() {
        let m = block_mesh(&multigrading_cube_dict(5)).unwrap();
        let back = PolyMesh::from_foam_poly_mesh(&m.to_foam_poly_mesh());
        assert_eq!(back.points, m.points);
        assert_eq!((back.n_cells, back.n_internal_faces, back.faces.len()), (m.n_cells, m.n_internal_faces, m.faces.len()));
        assert!(back.faces.iter().zip(&m.faces).all(|(a, b)| a.verts == b.verts && a.owner == b.owner && a.neighbour == b.neighbour));
        assert_eq!(back.patches, m.patches);
    }

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

    /// V&V (gh:#1364) — dev's integer `pow` and the unclamped last division.
    ///
    /// Methodology: `integer_pow_dev` is compared with the hand-unrolled
    /// squaring order of `Scalar.H` `IntegerPow` (`x^5 = x * (x^2)^2`, written
    /// out as `y = x; xx = x*x; xx = xx*xx; y *= xx`), and its special cases
    /// (`0`, `1`, `-1`, negative) are checked. For a 2-section edge with
    /// sections of equal length, the dev and v2506 flavours must both keep the
    /// computed last division (`s[4] + 0.5 * 4 / 4`, bit for bit) instead of
    /// assigning 1. The flavours must differ on a
    /// graded edge only through `pow` (real against integer), by at most a few
    /// ulp. Pass criterion: exact equality for the pow cases and the computed
    /// last division, flavour difference `<= 1e-15`.
    /// Results (measured 2026-11-11): all met; see the test body.
    #[test]
    fn dev_integer_pow_and_unclamped_last_division() {
        let x = 1.0173_f64;
        let x2 = x * x;
        let x4 = x2 * x2;
        assert_eq!(integer_pow_dev(x, 5).to_bits(), (x * x4).to_bits());
        assert_eq!(integer_pow_dev(x, 0), 1.0);
        assert_eq!(integer_pow_dev(x, 1), x);
        assert_eq!(integer_pow_dev(x, -1), 1.0 / x);
        let inv = 1.0 / x;
        assert_eq!(integer_pow_dev(x, -3).to_bits(), (inv * (inv * inv)).to_bits());
        let segs = vec![
            GradingSegment { fraction_length: 1.0, fraction_cells: 1.0, expansion: 2.0 },
            GradingSegment { fraction_length: 1.0, fraction_cells: 1.0, expansion: 1.0 },
        ];
        for fl in [ProjectionFlavour::Dev, ProjectionFlavour::Esi] {
            let s = graded_positions_multi(8, &segs, fl);
            // Two unit-length sections: the last node is secStart + blockFrac
            // with blockFrac = 0.5, so the raw value is 1, but it is computed,
            // not assigned: it must equal that computation bit for bit.
            assert_eq!(s[8].to_bits(), (s[4] + 0.5 * 4.0 / 4.0).to_bits());
        }
        let d = graded_positions_multi(10, &segs[..1], ProjectionFlavour::Dev);
        let e = graded_positions_multi(10, &segs[..1], ProjectionFlavour::Esi);
        let worst = d.iter().zip(&e).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        assert!(worst <= 1e-15, "flavours differ by {worst}");
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
        let s = graded_positions_multi(10, &segs, ProjectionFlavour::Dev);

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
        assert!(
            (w[1] / w[0] - 2.0).abs() < 1e-9,
            "seg0 ratio1 = {}",
            w[1] / w[0]
        );
        assert!(
            (w[2] / w[1] - 2.0).abs() < 1e-9,
            "seg0 ratio2 = {}",
            w[2] / w[1]
        );
        assert!(
            (w[2] / w[0] - 4.0).abs() < 1e-9,
            "seg0 end/start = {}",
            w[2] / w[0]
        );

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
        let a = graded_positions_multi(8, &seg, ProjectionFlavour::Dev);
        let b = graded_positions(8, 5.0);
        for i in 0..a.len() {
            assert!(
                (a[i] - b[i]).abs() < 1e-12,
                "node {i}: {} vs {}",
                a[i],
                b[i]
            );
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
        let verts = "vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );";
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

    /// V&V — genuinely per-edge `edgeGrading` parses to the 12-edge blend.
    ///
    /// Methodology: an `edgeGrading` whose four x-edges disagree `(1 3 2 5)`
    /// cannot collapse to one distribution per direction, so [`parse`] must
    /// keep the full 12 edge gradings (`edge_grading = Some(..)`) and the block
    /// build must use `edge_blended_node`. Reference: OpenFOAM
    /// `blockDescriptorEdges.C:121-141` (edges 0–3 = x). Pass criteria: the
    /// block's `edge_grading` is `Some`; its four x edges are
    /// `Uniform(1/3/2/5)`; y/z edges are `Uniform(1)`; the representative
    /// `grading` is `[Uniform(1); 3]` (first edge of each direction).
    ///
    /// Results (measured 2026-07-20): `edge_grading = Some`; x edges
    /// `[Uniform(1), Uniform(3), Uniform(2), Uniform(5)]`; y/z all `Uniform(1)`;
    /// representative grading `[Uniform(1), Uniform(1), Uniform(1)]`. Passed.
    #[test]
    fn edge_grading_differing_edges_parses_per_edge() {
        let dict = differing_edge_dict(2);
        let parsed = BlockMeshDict::parse(&dict).expect("parse per-edge edgeGrading");
        let block = &parsed.blocks[0];
        let eg = block
            .edge_grading
            .as_ref()
            .expect("differing edges must retain a per-edge Some(..)");
        assert_eq!(eg[0], Grading::Uniform(1.0));
        assert_eq!(eg[1], Grading::Uniform(3.0));
        assert_eq!(eg[2], Grading::Uniform(2.0));
        assert_eq!(eg[3], Grading::Uniform(5.0));
        for e in 4..12 {
            assert_eq!(eg[e], Grading::Uniform(1.0), "edge {e}");
        }
        assert_eq!(
            block.grading,
            [
                Grading::Uniform(1.0),
                Grading::Uniform(1.0),
                Grading::Uniform(1.0)
            ]
        );
    }

    /// A unit-cube dict `[0,1]^3` whose four x-edges (OpenFOAM edges 0,1,2,3)
    /// carry the *different* ratios `1, 3, 2, 5` and whose y/z edges are all
    /// uniform. `nx` x-cells, 1 cell in y and z. This is the canonical case the
    /// old code could not represent (four distinct x distributions).
    fn differing_edge_dict(nx: usize) -> String {
        format!(
            "convertToMeters 1;\n\
             vertices ( (0 0 0)(1 0 0)(1 1 0)(0 1 0)(0 0 1)(1 0 1)(1 1 1)(0 1 1) );\n\
             blocks ( hex (0 1 2 3 4 5 6 7) ({nx} 1 1) \
             edgeGrading ( 1 3 2 5  1 1 1 1  1 1 1 1 ) );\nboundary ( );\n"
        )
    }

    /// Locate a mesh point within `1e-9 m` of `target`.
    fn has_point(m: &PolyMesh, target: Vector3) -> bool {
        m.points.iter().any(|p| (*p - target).mag() < 1e-9)
    }

    /// V&V — per-edge `edgeGrading` places each x-edge's mid-node by its OWN
    /// grading (exact hand reference).
    ///
    /// Methodology: build the unit cube of [`differing_edge_dict`] with
    /// `nx = 2, ny = nz = 1`. Every node then lies on a block edge, and the
    /// blend of OpenFOAM `block::createPoints` (`blockCreate.C:45-183`) reduces
    /// exactly to the single edge's `lambdaDivisions` at that node (algebraic
    /// reduction; verified by hand for node `(i=1,j=1,k=0)` on edge 1). The four
    /// mid-plane nodes `(i=1, *, *)` thus take the `[0,1]` mid position of their
    /// respective x-edge:
    ///
    /// - `(1,0,0)` → edge 0, ratio 1 → `graded_positions(2,1)[1] = 0.5`
    /// - `(1,1,0)` → edge 1, ratio 3 → `graded_positions(2,3)[1] = 0.25`
    /// - `(1,1,1)` → edge 2, ratio 2 → `graded_positions(2,2)[1] = 1/3`
    /// - `(1,0,1)` → edge 3, ratio 5 → `graded_positions(2,5)[1] = 1/6`
    ///
    /// Reference: the analytic geometric-series node positions and the
    /// edge→corner map (edge 0 = corners 0-1, edge 1 = 3-2, edge 2 = 7-6,
    /// edge 3 = 4-5). Pass criteria: the mesh carries points at
    /// `(0.5, 0, 0)`, `(0.25, 1, 0)`, `(1/3, 1, 1)`, `(1/6, 0, 1)` (tol
    /// `1e-9`); 2 cells; total volume `= 1 m^3`; `FvMesh::validate` OK. This is
    /// exactly what a single distribution-per-direction map cannot do — the
    /// four x positions at the mid plane differ.
    ///
    /// Results (measured 2026-07-20): all four target points present; 2 cells;
    /// total volume `1.0 m^3` (error `< 1e-12`); validate OK. Passed.
    #[test]
    fn edge_grading_per_edge_exact_nodes() {
        let mesh = block_mesh(&differing_edge_dict(2)).expect("build per-edge");
        assert_eq!(mesh.n_cells, 2);

        // Each x-edge mid-node reflects its own grading's mid position.
        assert!(has_point(&mesh, Vector3::new(0.5, 0.0, 0.0)), "edge0 mid");
        assert!(has_point(&mesh, Vector3::new(0.25, 1.0, 0.0)), "edge1 mid");
        assert!(
            has_point(&mesh, Vector3::new(1.0 / 3.0, 1.0, 1.0)),
            "edge2 mid"
        );
        assert!(
            has_point(&mesh, Vector3::new(1.0 / 6.0, 0.0, 1.0)),
            "edge3 mid"
        );

        // Grading only moves nodes: the (warped-internal-face) mesh still fills
        // the unit cube exactly.
        assert!(
            (mesh.total_volume() - 1.0).abs() < 1e-12,
            "volume = {}",
            mesh.total_volume()
        );
        let fv = mesh.to_fv_mesh().expect("to_fv_mesh");
        assert!(fv.validate().is_ok());
    }

    /// V&V — the 12-edge blend reduces *exactly* to trilinear when the four
    /// edges of each direction agree.
    ///
    /// Methodology: OpenFOAM's `block::createPoints` blend is a strict
    /// generalisation of trilinear interpolation — for equal per-direction edge
    /// distributions the normalised 4-edge blend in each direction collapses to
    /// the bilinear/trilinear map (`blockCreate.C:92-152`, proven in the
    /// `edge_blended_node` docs). We build the 12 edge lambda arrays for a
    /// unit cube with all four x-edges = ratio 4 and uniform y/z (`4x4x4`
    /// cells), then compare `edge_blended_node` against [`trilinear`] with the
    /// per-direction distributions at every one of the `5x5x5 = 125` grid nodes.
    /// Reference/pass criterion: max node discrepancy `< 1e-12 m` — the two
    /// independent code paths must agree to round-off.
    ///
    /// Results (measured 2026-07-20): over all 125 nodes the maximum
    /// `|blend - trilinear|` was `< 1e-12 m` (agreement to floating-point
    /// round-off). This validates the blend as a correct generalisation and
    /// shows why the equal-edges fast path stays bit-comparable. Passed.
    #[test]
    fn edge_blend_reduces_to_trilinear() {
        let corners: [Vector3; 8] = std::array::from_fn(|c| {
            Vector3::new(HEX_CORNER[c][0], HEX_CORNER[c][1], HEX_CORNER[c][2])
        });

        // All four x-edges share ratio 4; y and z uniform.
        let mut eg: [Grading; 12] = std::array::from_fn(|_| Grading::Uniform(1.0));
        for e in eg.iter_mut().take(4) {
            *e = Grading::Uniform(4.0);
        }
        let cells = [4usize, 4, 4];
        let el = edge_lambdas(&eg, cells);

        let ux = Grading::Uniform(4.0).node_positions(4);
        let uy = Grading::Uniform(1.0).node_positions(4);
        let uz = Grading::Uniform(1.0).node_positions(4);

        let mut max_err = 0.0f64;
        for k in 0..=4 {
            for j in 0..=4 {
                for i in 0..=4 {
                    let blended = edge_blended_node(&corners, &el, i, j, k);
                    let tri = trilinear(&corners, ux[i], uy[j], uz[k]);
                    max_err = max_err.max((blended - tri).mag());
                }
            }
        }
        assert!(max_err < 1e-12, "max blend-vs-trilinear error = {max_err}");
    }

    /// V&V — a per-edge-graded block builds a valid, volume-preserving mesh at
    /// higher resolution.
    ///
    /// Methodology: build [`differing_edge_dict`] with `nx = 8` (8x1x1). The
    /// four x-edges grade differently, so the mid-block node planes warp, but
    /// the structured hex grid still tiles the unit cube. Reference: analytic
    /// block volume `1 m^3`; pass criteria: 8 cells; `FvMesh::validate` OK;
    /// total volume `= 1 m^3`; every cell volume strictly positive (no inverted
    /// cells).
    ///
    /// Results (measured 2026-07-20): 8 cells; validate OK; total volume
    /// `1.0 m^3` (error `< 1e-12`); min cell volume `> 0`. Passed.
    #[test]
    fn edge_grading_per_edge_build_volume() {
        let mesh = block_mesh(&differing_edge_dict(8)).expect("build per-edge 8");
        assert_eq!(mesh.n_cells, 8);
        assert!(
            (mesh.total_volume() - 1.0).abs() < 1e-12,
            "volume = {}",
            mesh.total_volume()
        );
        let fv = mesh.to_fv_mesh().expect("to_fv_mesh");
        assert!(fv.validate().is_ok());
        // No inverted cells.
        for (c, &v) in fv.cell_volumes.iter().enumerate() {
            assert!(v > 0.0, "cell {c} has non-positive volume {v}");
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
