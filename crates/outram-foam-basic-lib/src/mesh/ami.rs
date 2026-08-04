// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
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

//! Arbitrary Mesh Interface (AMI) weight computation and non-conformal periodic
//! (cyclicAMI) coupling representation.
//!
//! Mirrors OpenFOAM's
//! `src/meshTools/AMIInterpolation/AMIInterpolation/AMIInterpolation.H`
//! (the geometric face-overlap weighting) and
//! `src/finiteVolume/fields/fvPatchFields/constraint/cyclicAMI/cyclicAMIFvPatchField.H`
//! (the coupled-interface contribution), together with the
//! `cyclicAMIPolyPatch` topology.
//!
//! ## What AMI is (and why it differs from plain cyclic)
//!
//! A plain [`PatchKind::Cyclic`](crate::mesh::PatchKind::Cyclic) patch pair is
//! **conformal**: local face `i` of one half matches local face `i` of the
//! other exactly one-to-one, so the seam is discretised like an ordinary
//! internal face (see [`CyclicCoupling`](crate::mesh::CyclicCoupling)).
//!
//! A [`PatchKind::CyclicAmi`](crate::mesh::PatchKind::CyclicAmi) pair is
//! **non-conformal**: the two halves' faces do *not* line up, so each *target*
//! face overlaps several *source* faces. The coupling for one target face is
//! therefore a **weighted set** of source cells, the weight of each being the
//! geometric overlap-area fraction
//! `w_k = overlap_area(target, source_k) / target_area`.
//! When a target is fully covered by sources these weights sum to `1`
//! (conservative interpolation), so the value seen across the seam is the
//! area-weighted average of the overlapping source cells.
//!
//! ## Overlap method implemented here (first pass — planar / 1-D structured)
//!
//! [`overlap_weights_1d`] projects both patch halves onto a common seam plane
//! and treats each face as an **interval along a single transverse axis** of
//! constant out-of-plane depth (a structured 2-D seam). The overlap of a target
//! interval `[t0, t1]` with a source interval `[s0, s1]` is the 1-D segment
//! overlap `max(0, min(t1, s1) - max(t0, s0))`, multiplied by the constant
//! `depth` to give an overlap **area** [m²]. This is exact for axis-aligned,
//! coplanar, structured seams (e.g. a translational-periodic channel meshed with
//! differing transverse resolutions on the two halves) — the case this first
//! pass targets.
//!
//! ### Deferred (documented limitations)
//!
//! - **General 3-D polygon clipping.** True `AMIInterpolation` clips arbitrary
//!   source polygons against each target polygon (Sutherland-Hodgman /
//!   greatest-area walk). That is *not* implemented here; only the 1-D interval
//!   overlap above is. Non-axis-aligned faces, skewed seams, and unstructured
//!   transverse tilings are out of scope for this pass.
//! - **Two transverse axes.** Only one transverse coordinate is overlapped; a
//!   fully 2-D tiled seam (subdivided in both in-plane directions) is not
//!   handled.
//! - **Non-planar / curved seams and per-face normal rotation** (`cyclicAMI`
//!   with a rotational transform) are not handled.
//!
//! These limits are acceptable for the verification cases this module ships
//! (matching-mesh limit reproduces plain cyclic; a 2:1 non-conformal case is
//! conservative). This code is an **untrusted AI-assisted draft pending human
//! V&V review** (2026-08-04).

use crate::mesh::fv_mesh::{
    BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind,
};
use crate::primitives::Vector3;

/// One overlap between a target face and a source face on an AMI seam.
///
/// Produced by [`overlap_weights_1d`]; purely geometric (carries the *local*
/// source-face index within the source patch, not a global face or cell index —
/// the mesh constructor attaches those when it builds an [`AmiWeight`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmiOverlap {
    /// Local index of the overlapping source face within the source patch.
    pub source: usize,
    /// Geometric overlap area between the two faces [m²].
    pub overlap_area: f64,
    /// Overlap fraction of the **target** face:
    /// `overlap_area / target_area` (dimensionless). Summed over all sources of
    /// one target this is `1` when the target is fully covered.
    pub weight: f64,
}

/// One weighted source-cell contribution to a single AMI target seam face.
///
/// The finite-volume operators treat each [`AmiWeight`] as one "partial internal
/// face" of area [`overlap_area`](Self::overlap_area) joining the target cell to
/// [`source_cell`](Self::source_cell): the off-diagonal seam coefficient is
/// scaled by this pair's overlap so the whole target face's flux is distributed
/// conservatively across its overlapping sources.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmiWeight {
    /// Global face index of the overlapped source face.
    pub source_face: usize,
    /// Owner cell of the source face — the "neighbour" across this partial seam.
    pub source_cell: usize,
    /// Overlap fraction of the target face (`overlap_area / target_area`,
    /// dimensionless). Per target these sum to `≈ 1` (conservative).
    pub weight: f64,
    /// Geometric overlap area of this target/source pair [m²]. Used as the
    /// effective face area of the partial seam face in the diffusion/advection
    /// coefficient.
    pub overlap_area: f64,
}

/// One target seam face of a [`PatchKind::CyclicAmi`](crate::mesh::PatchKind::CyclicAmi)
/// patch pair, together with the weighted set of source cells it couples to.
///
/// Mirrors the coupled-interface contribution of `Foam::cyclicAMIFvPatchField`
/// whose `patchNeighbourField()` supplies the *interpolated* partner value
/// `Σ_k w_k · φ(source_cell_k)`.
///
/// The couplings are appended to the LDU face addressing *after* the internal
/// faces and the [`CyclicCoupling`](crate::mesh::CyclicCoupling)s: one LDU face
/// per [`AmiWeight`], laid out in `ami_couplings` order (see
/// [`FvMesh::ami_ldu_start`](crate::mesh::FvMesh::ami_ldu_start) and
/// [`FvMesh::n_ami_faces`](crate::mesh::FvMesh::n_ami_faces)).
#[derive(Debug, Clone, PartialEq)]
pub struct AmiCoupling {
    /// Global face index of this target seam face.
    pub target_face: usize,
    /// Owner cell of the target face — the "owner" side of every partial seam
    /// face in [`weights`](Self::weights).
    pub target_cell: usize,
    /// Patch index of the target half of the AMI pair.
    pub target_patch: usize,
    /// Patch index of the source half of the AMI pair.
    pub source_patch: usize,
    /// Local face index of the target face within its patch
    /// (`target_face - patches[target_patch].start`).
    pub local: usize,
    /// Weighted source contributions; per-target weights sum to `≈ 1`.
    pub weights: Vec<AmiWeight>,
}

impl AmiCoupling {
    /// Sum of this target's overlap weights. Equals `1` (to rounding) when the
    /// target face is fully covered by source faces — the conservation check.
    pub fn weight_sum(&self) -> f64 {
        self.weights.iter().map(|w| w.weight).sum()
    }
}

/// Overlap of two closed intervals `[a0, a1]` and `[b0, b1]` (0 if disjoint).
#[inline]
fn interval_overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Planar / 1-D-structured AMI overlap weights.
///
/// Given a target patch and a source patch each described as a list of
/// **transverse intervals** `(lo, hi)` (the projection of each face onto a
/// single in-plane axis of the shared seam plane) plus the constant out-of-plane
/// `depth` [m], return for every target face the list of [`AmiOverlap`]s with
/// the source faces it geometrically overlaps.
///
/// - `target_spans[i] = (t_lo, t_hi)` — transverse extent of target face `i` [m].
/// - `source_spans[j] = (s_lo, s_hi)` — transverse extent of source face `j` [m].
/// - `depth` — constant out-of-plane face depth [m] (`> 0`).
///
/// The overlap **area** of target `i` with source `j` is
/// `interval_overlap · depth` [m²]; the **weight** is that area divided by the
/// target face's own area `(t_hi - t_lo)·depth`, i.e. simply the fraction of the
/// target interval covered by the source interval. Sources with zero overlap are
/// omitted. When the target intervals are fully tiled by the source intervals
/// (full coverage) each target's weights sum to `1`.
///
/// # Panics
/// Panics if `depth <= 0` or if any target span is degenerate (`hi <= lo`).
///
/// # Example
/// ```
/// use outram_foam_basic_lib::mesh::ami::overlap_weights_1d;
/// // One coarse target [0,1] over two fine sources [0,0.5], [0.5,1], depth 1.
/// let w = overlap_weights_1d(&[(0.0, 1.0)], &[(0.0, 0.5), (0.5, 1.0)], 1.0);
/// assert_eq!(w[0].len(), 2);
/// assert!((w[0][0].weight - 0.5).abs() < 1e-15);
/// assert!((w[0][1].weight - 0.5).abs() < 1e-15);
/// // Conservative: weights sum to 1.
/// let s: f64 = w[0].iter().map(|o| o.weight).sum();
/// assert!((s - 1.0).abs() < 1e-15);
/// ```
pub fn overlap_weights_1d(
    target_spans: &[(f64, f64)],
    source_spans: &[(f64, f64)],
    depth: f64,
) -> Vec<Vec<AmiOverlap>> {
    assert!(depth > 0.0, "AMI depth must be positive, got {depth}");
    let mut out = Vec::with_capacity(target_spans.len());
    for &(t0, t1) in target_spans {
        assert!(t1 > t0, "degenerate target span ({t0}, {t1})");
        let t_len = t1 - t0;
        let t_area = t_len * depth;
        let mut row = Vec::new();
        for (j, &(s0, s1)) in source_spans.iter().enumerate() {
            let ov_len = interval_overlap(t0, t1, s0, s1);
            if ov_len <= 0.0 {
                continue;
            }
            let overlap_area = ov_len * depth;
            row.push(AmiOverlap {
                source: j,
                overlap_area,
                weight: overlap_area / t_area,
            });
        }
        out.push(row);
    }
    out
}

impl FvMesh {
    /// First LDU face index occupied by AMI seam couplings.
    ///
    /// AMI partial-seam faces live *after* the internal faces and the
    /// [`CyclicCoupling`](crate::mesh::CyclicCoupling)s, so the AMI block starts
    /// at `n_internal_faces + cyclic_couplings.len()`. Operators and
    /// [`FvMatrix::new`](crate::ldu_matrix::FvMatrix::new) both iterate the AMI
    /// couplings in order from this base, one LDU face per [`AmiWeight`], so the
    /// addressing lines up.
    pub fn ami_ldu_start(&self) -> usize {
        self.n_internal_faces + self.cyclic_couplings.len()
    }

    /// Total number of AMI partial-seam LDU faces — the sum of each AMI target
    /// face's number of weighted source contributions.
    pub fn n_ami_faces(&self) -> usize {
        self.ami_couplings.iter().map(|c| c.weights.len()).sum()
    }

    /// Build a **non-conformal periodic ring** with two `cyclicAMI` seams,
    /// programmatically (no `polyMesh` parser).
    ///
    /// The mesh is a closed loop of two cell columns, periodic in x, whose two
    /// column-joining interfaces are both AMI seams so the two halves may be
    /// subdivided differently in the transverse (y) direction — the defining
    /// non-conformality AMI exists to handle:
    ///
    /// ```text
    ///        wrap seam (x = 0 ≡ x = lx, period +lx·x̂)
    ///   ┌──────────────────────────────────────────────┐
    ///   │                                               │
    ///  A column (n_a cells in y)     B column (n_b cells in y)
    ///   │      x ∈ [0, lx/2]           x ∈ [lx/2, lx]   │
    ///   └────────── mid seam (x = lx/2) ────────────────┘
    /// ```
    ///
    /// - **A column** — `n_a` cells stacked in `y ∈ [0, ly]`, centres at
    ///   `x = lx/4`; each cell owns a mid-seam face (`x = lx/2`, target of the
    ///   *mid* AMI) and a wrap-seam face (`x = 0`, source of the *wrap* AMI).
    /// - **B column** — `n_b` cells stacked in `y ∈ [0, ly]`, centres at
    ///   `x = 3lx/4`; each cell owns a mid-seam face (`x = lx/2`, source of the
    ///   *mid* AMI) and a wrap-seam face (`x = lx`, target of the *wrap* AMI).
    ///
    /// There are **no internal faces**: every coupling is through one of the two
    /// AMI seams. Each cell has equal in-/out-seam face area, so a uniform field
    /// advects around the loop conservatively (`A·1 = 0`), and the diffusion
    /// stencil is the periodic circulant one. Cell-to-cell distance across either
    /// seam is `lx/2` (a quarter-width from each owner to the seam plane on both
    /// sides), matching an interior face of that spacing.
    ///
    /// When `n_a == n_b` **and** the two halves align (equal transverse
    /// tiling), every AMI weight is `1` and the couplings reduce to the plain
    /// [`CyclicCoupling`](crate::mesh::CyclicCoupling) one-to-one seam — the
    /// mesh then decomposes into `n_a` independent 2-cell periodic loops, each
    /// identical to [`periodic_1d`](FvMesh::periodic_1d)`(2, lx, (ly/n_a)·depth)`
    /// (the matching-mesh limit used in this crate's AMI V&V).
    ///
    /// # Parameters
    /// - `n_a` — transverse cell count of the A (mid-target / wrap-source) column
    ///   (`≥ 1`);
    /// - `n_b` — transverse cell count of the B (mid-source / wrap-target) column
    ///   (`≥ 1`);
    /// - `lx` — periodic (x) domain length [m] (`> 0`);
    /// - `ly` — transverse (y) domain extent [m] (`> 0`);
    /// - `depth` — out-of-plane (z) face depth [m] (`> 0`).
    ///
    /// Cells are ordered `A_0 … A_{n_a-1}, B_0 … B_{n_b-1}`. Patches are, in
    /// order: `"A_right"` (mid target), `"B_left"` (mid source), `"B_right"`
    /// (wrap target), `"A_left"` (wrap source), all [`PatchKind::CyclicAmi`].
    ///
    /// # Panics
    /// Panics if any of `n_a`, `n_b` is `0` or any of `lx`, `ly`, `depth` is not
    /// positive.
    ///
    /// OpenFOAM analogue: a `blockMesh` with a `cyclicAMI` patch pair on two
    /// non-conformal column interfaces.
    pub fn periodic_ring_ami(n_a: usize, n_b: usize, lx: f64, ly: f64, depth: f64) -> FvMesh {
        assert!(n_a >= 1 && n_b >= 1, "periodic_ring_ami needs n_a,n_b ≥ 1");
        assert!(lx > 0.0 && ly > 0.0 && depth > 0.0, "lx, ly, depth must be > 0");
        let dy_a = ly / n_a as f64;
        let dy_b = ly / n_b as f64;
        let a_area = dy_a * depth; // area of one A-column seam face [m²]
        let b_area = dy_b * depth; // area of one B-column seam face [m²]
        let n_cells = n_a + n_b;

        // Cell centres and volumes.
        let mut cell_centres = Vec::with_capacity(n_cells);
        let mut cell_volumes = Vec::with_capacity(n_cells);
        for i in 0..n_a {
            cell_centres.push(Vector3::new(0.25 * lx, (i as f64 + 0.5) * dy_a, 0.0));
            cell_volumes.push(0.5 * lx * dy_a * depth);
        }
        for j in 0..n_b {
            cell_centres.push(Vector3::new(0.75 * lx, (j as f64 + 0.5) * dy_b, 0.0));
            cell_volumes.push(0.5 * lx * dy_b * depth);
        }

        // Face layout (all boundary; no internal faces):
        //   patch 0 "A_right"  faces [0            , n_a)          owner A_i, +x @ x=lx/2
        //   patch 1 "B_left"   faces [n_a          , n_a+n_b)      owner B_j, -x @ x=lx/2
        //   patch 2 "B_right"  faces [n_a+n_b      , n_a+2n_b)     owner B_j, +x @ x=lx
        //   patch 3 "A_left"   faces [n_a+2n_b     , 2n_a+2n_b)    owner A_i, -x @ x=0
        let p0 = 0;
        let p1 = n_a;
        let p2 = n_a + n_b;
        let p3 = n_a + 2 * n_b;
        let n_faces = 2 * n_a + 2 * n_b;

        let mut owner = vec![0usize; n_faces];
        let mut face_centres = vec![Vector3::ZERO; n_faces];
        let mut face_area_vectors = vec![Vector3::ZERO; n_faces];

        for i in 0..n_a {
            let yc = (i as f64 + 0.5) * dy_a;
            // A_right (mid target): +x at x=lx/2
            owner[p0 + i] = i;
            face_centres[p0 + i] = Vector3::new(0.5 * lx, yc, 0.0);
            face_area_vectors[p0 + i] = Vector3::new(a_area, 0.0, 0.0);
            // A_left (wrap source): -x at x=0
            owner[p3 + i] = i;
            face_centres[p3 + i] = Vector3::new(0.0, yc, 0.0);
            face_area_vectors[p3 + i] = Vector3::new(-a_area, 0.0, 0.0);
        }
        for j in 0..n_b {
            let yc = (j as f64 + 0.5) * dy_b;
            let cell = n_a + j;
            // B_left (mid source): -x at x=lx/2
            owner[p1 + j] = cell;
            face_centres[p1 + j] = Vector3::new(0.5 * lx, yc, 0.0);
            face_area_vectors[p1 + j] = Vector3::new(-b_area, 0.0, 0.0);
            // B_right (wrap target): +x at x=lx
            owner[p2 + j] = cell;
            face_centres[p2 + j] = Vector3::new(lx, yc, 0.0);
            face_area_vectors[p2 + j] = Vector3::new(b_area, 0.0, 0.0);
        }

        // Transverse spans for overlap.
        let a_spans: Vec<(f64, f64)> =
            (0..n_a).map(|i| (i as f64 * dy_a, (i as f64 + 1.0) * dy_a)).collect();
        let b_spans: Vec<(f64, f64)> =
            (0..n_b).map(|j| (j as f64 * dy_b, (j as f64 + 1.0) * dy_b)).collect();

        // Mid AMI: target = A_right (patch 0), source = B_left (patch 1).
        let mid_overlaps = overlap_weights_1d(&a_spans, &b_spans, depth);
        let mut ami_couplings = Vec::with_capacity(n_a + n_b);
        for (i, ov_row) in mid_overlaps.iter().enumerate() {
            let weights = ov_row
                .iter()
                .map(|o| AmiWeight {
                    source_face: p1 + o.source,
                    source_cell: n_a + o.source,
                    weight: o.weight,
                    overlap_area: o.overlap_area,
                })
                .collect();
            ami_couplings.push(AmiCoupling {
                target_face: p0 + i,
                target_cell: i,
                target_patch: 0,
                source_patch: 1,
                local: i,
                weights,
            });
        }

        // Wrap AMI: target = B_right (patch 2), source = A_left (patch 3),
        // period offset +lx·x̂ (handled implicitly — the transverse spans are
        // already in the shared seam-plane coordinate).
        let wrap_overlaps = overlap_weights_1d(&b_spans, &a_spans, depth);
        for (j, ov_row) in wrap_overlaps.iter().enumerate() {
            let weights = ov_row
                .iter()
                .map(|o| AmiWeight {
                    source_face: p3 + o.source,
                    source_cell: o.source,
                    weight: o.weight,
                    overlap_area: o.overlap_area,
                })
                .collect();
            ami_couplings.push(AmiCoupling {
                target_face: p2 + j,
                target_cell: n_a + j,
                target_patch: 2,
                source_patch: 3,
                local: j,
                weights,
            });
        }

        let patches = vec![
            BoundaryPatch::new("A_right", p0, n_a, PatchKind::CyclicAmi),
            BoundaryPatch::new("B_left", p1, n_b, PatchKind::CyclicAmi),
            BoundaryPatch::new("B_right", p2, n_b, PatchKind::CyclicAmi),
            BoundaryPatch::new("A_left", p3, n_a, PatchKind::CyclicAmi),
        ];

        FvMeshBuilder::new()
            .n_cells(n_cells)
            .n_internal_faces(0)
            .owner(owner)
            .neighbour(vec![])
            .patches(patches)
            .ami_couplings(ami_couplings)
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .expect("periodic_ring_ami builds a consistent mesh")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (verification, 2026-08-04). Overlap-weight unit test vs hand-computed
    /// areas — a coarse target over aligned fine sources.
    ///
    /// Methodology: one target interval `[0, 1]`, depth `2`, over four equal
    /// sources `[0,0.25],[0.25,0.5],[0.5,0.75],[0.75,1]`. Each overlap length is
    /// `0.25`, so each overlap area is `0.25·2 = 0.5` [m²] and each weight is
    /// `0.5·2 / (1·2) = 0.25`. Pass criterion: four sources, each area `0.5`,
    /// each weight `0.25`, weights sum to `1`, all to < 1e-15.
    /// Result (measured 2026-08-04): areas = [0.5,0.5,0.5,0.5], weights =
    /// [0.25,0.25,0.25,0.25], Σw = 1.0 (exact). PASS.
    #[test]
    fn vv_overlap_weights_coarse_over_fine() {
        let src = [(0.0, 0.25), (0.25, 0.5), (0.5, 0.75), (0.75, 1.0)];
        let w = overlap_weights_1d(&[(0.0, 1.0)], &src, 2.0);
        assert_eq!(w[0].len(), 4);
        let mut sum = 0.0;
        for (k, o) in w[0].iter().enumerate() {
            assert_eq!(o.source, k);
            assert!((o.overlap_area - 0.5).abs() < 1e-15, "area {}", o.overlap_area);
            assert!((o.weight - 0.25).abs() < 1e-15, "weight {}", o.weight);
            sum += o.weight;
        }
        assert!((sum - 1.0).abs() < 1e-15, "Σw = {sum}");
    }

    /// V&V (verification, 2026-08-04). Partial-overlap weight vs hand value.
    ///
    /// Methodology: target `[0.2, 0.8]` (length 0.6) over sources `[0,0.5]` and
    /// `[0.5,1.0]`, depth 1. Overlap with source 0 is `[0.2,0.5]` = 0.3; with
    /// source 1 is `[0.5,0.8]` = 0.3. Weights = 0.3/0.6 = 0.5 each; areas 0.3.
    /// Pass criterion: matches to < 1e-15 and Σw = 1.
    /// Result (measured 2026-08-04): areas [0.3,0.3], weights [0.5,0.5], Σw = 1.
    /// PASS.
    #[test]
    fn vv_overlap_weights_partial() {
        let w = overlap_weights_1d(&[(0.2, 0.8)], &[(0.0, 0.5), (0.5, 1.0)], 1.0);
        assert_eq!(w[0].len(), 2);
        assert!((w[0][0].overlap_area - 0.3).abs() < 1e-15);
        assert!((w[0][1].overlap_area - 0.3).abs() < 1e-15);
        assert!((w[0][0].weight - 0.5).abs() < 1e-15);
        assert!((w[0][1].weight - 0.5).abs() < 1e-15);
        let s: f64 = w[0].iter().map(|o| o.weight).sum();
        assert!((s - 1.0).abs() < 1e-15);
    }

    /// Disjoint source contributes nothing (no spurious overlap).
    #[test]
    fn overlap_disjoint_is_empty() {
        let w = overlap_weights_1d(&[(0.0, 0.5)], &[(0.6, 1.0)], 1.0);
        assert!(w[0].is_empty());
    }

    /// V&V (verification, 2026-08-04). `periodic_ring_ami` topology + per-target
    /// weight-sum conservation on a 2:1 non-conformal seam.
    ///
    /// Methodology: `periodic_ring_ami(2, 4, 1.0, 1.0, 1.0)` — 2 coarse A cells
    /// (mid targets) over 4 fine B cells (mid sources), i.e. a 2:1 target:source
    /// ratio, plus the wrap seam (4 B targets over 2 A sources, a 1:2 ratio). It
    /// checks: 6 cells; 4 CyclicAmi patches; every AMI target's weights sum to 1
    /// (conservative); and the coarse mid target overlaps exactly 2 fine sources.
    /// Pass criterion: all counts exact, every `weight_sum()` within 1e-14 of 1.
    /// Result (measured 2026-08-04): 6 cells, 4 patches, 6 AMI targets, each
    /// weight_sum = 1.0; mid target 0 → 2 sources of weight 0.5. PASS.
    #[test]
    fn vv_ring_topology_and_weight_sum() {
        let m = FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0);
        assert_eq!(m.n_cells, 6);
        assert_eq!(m.n_internal_faces, 0);
        assert_eq!(m.patches.len(), 4);
        for p in &m.patches {
            assert_eq!(p.kind, PatchKind::CyclicAmi);
        }
        // 2 mid targets (A) + 4 wrap targets (B) = 6 AMI couplings.
        assert_eq!(m.ami_couplings.len(), 6);
        for cc in &m.ami_couplings {
            assert!(
                (cc.weight_sum() - 1.0).abs() < 1e-14,
                "target {} weight_sum = {}",
                cc.target_face,
                cc.weight_sum()
            );
        }
        // Mid target A_0 overlaps B_left 0 and 1 (each weight 0.5).
        let mid0 = &m.ami_couplings[0];
        assert_eq!(mid0.target_cell, 0);
        assert_eq!(mid0.weights.len(), 2);
        for w in &mid0.weights {
            assert!((w.weight - 0.5).abs() < 1e-14);
        }
        // n_ami_faces = mid (2·2) + wrap (4·1) = 8.
        assert_eq!(m.n_ami_faces(), 8);
        assert_eq!(m.ami_ldu_start(), 0); // no internal faces, no cyclic couplings
    }

    /// Matching ring (n_a == n_b, aligned) has identity weights (one source per
    /// target, weight 1) — the reduction to plain cyclic.
    #[test]
    fn vv_ring_matching_is_identity() {
        let m = FvMesh::periodic_ring_ami(3, 3, 1.0, 1.0, 1.0);
        assert_eq!(m.ami_couplings.len(), 6);
        for cc in &m.ami_couplings {
            assert_eq!(cc.weights.len(), 1, "matching target should have one source");
            assert!((cc.weights[0].weight - 1.0).abs() < 1e-14);
        }
    }
}
