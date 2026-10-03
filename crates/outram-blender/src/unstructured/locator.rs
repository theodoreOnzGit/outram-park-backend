// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! A uniform bucket grid over cell bounding boxes: the spatial index that
//! turns "which cell contains this point" from a scan of every cell into a
//! scan of a handful.
//!
//! # Where this sits relative to upstream
//!
//! OpenMC delegates the same job to a library: MOAB's `AdaptiveKDTree` over
//! the tets and their triangles (`MOABMesh::build_kdtree`,
//! `src/mesh.cpp:3099`, options `MAX_DEPTH=20;PLANE_SET=2`) or libMesh's
//! `PointLocator` (`LibMesh::get_bin`, `src/mesh.cpp:3973`). Neither is
//! available in pure Rust, so this is the pure-Rust equivalent: a uniform
//! grid, which is simpler than a k-d tree and as good for the near-uniform
//! cell sizes a reactor mesh has. It is an index only — it answers "which
//! cells might contain this point", never "which cell does"; the exact test is
//! the caller's (outram-mc-libs' `UnstructuredMeshExt`).
//!
//! The grid is part of the description (built once, read-only, shared through
//! the mesh's `Arc`) so that every consumer uses the same candidates.

/// Upper bound on buckets per axis, so a pathological aspect ratio cannot
/// allocate an unbounded grid.
const MAX_PER_AXIS: usize = 128;

/// Uniform bucket grid over cell bounding boxes (compressed-row storage).
#[derive(Debug, Clone, PartialEq)]
pub struct CellLocator {
    lower: [f64; 3],
    width: [f64; 3],
    dims: [usize; 3],
    /// `starts[b]..starts[b + 1]` indexes `items` for bucket `b`.
    starts: Vec<usize>,
    /// Cell indices, bucket by bucket.
    items: Vec<usize>,
}

impl CellLocator {
    /// Build the grid over `cell_bounds` (`[lower, upper]` per cell), which
    /// together span `[lower, upper]`.
    pub fn build(lower: [f64; 3], upper: [f64; 3], cell_bounds: &[[[f64; 3]; 2]]) -> Self {
        let n = cell_bounds.len().max(1);
        let ext = [upper[0] - lower[0], upper[1] - lower[1], upper[2] - lower[2]];
        // Target about one cell per bucket: h is the edge of a cube holding
        // the mean cell's share of the box (over the non-degenerate axes).
        let live: Vec<f64> = ext.iter().copied().filter(|e| *e > 0.0).collect();
        let h = if live.is_empty() {
            1.0
        } else {
            let vol: f64 = live.iter().product();
            (vol / n as f64).powf(1.0 / live.len() as f64)
        };
        let mut dims = [1usize; 3];
        let mut width = [1.0f64; 3];
        for a in 0..3 {
            if ext[a] > 0.0 && h > 0.0 {
                dims[a] = ((ext[a] / h).round() as usize).clamp(1, MAX_PER_AXIS);
                width[a] = ext[a] / dims[a] as f64;
            } else {
                dims[a] = 1;
                width[a] = if ext[a] > 0.0 { ext[a] } else { 1.0 };
            }
        }
        let n_buckets = dims[0] * dims[1] * dims[2];
        let mut grid = Self {
            lower,
            width,
            dims,
            starts: vec![0; n_buckets + 1],
            items: Vec::new(),
        };
        // Two passes: count, then fill (deterministic order: cell index).
        let mut counts = vec![0usize; n_buckets];
        for b in cell_bounds {
            grid.for_each_bucket_in_box(b[0], b[1], |k| counts[k] += 1);
        }
        for k in 0..n_buckets {
            grid.starts[k + 1] = grid.starts[k] + counts[k];
        }
        grid.items = vec![0; grid.starts[n_buckets]];
        let mut fill = grid.starts.clone();
        for (c, b) in cell_bounds.iter().enumerate() {
            let (lo, hi) = (b[0], b[1]);
            let (i0, i1) = (grid.clamp_index(lo), grid.clamp_index(hi));
            for k in i0[2]..=i1[2] {
                for j in i0[1]..=i1[1] {
                    for i in i0[0]..=i1[0] {
                        let bucket = grid.flat(i, j, k);
                        grid.items[fill[bucket]] = c;
                        fill[bucket] += 1;
                    }
                }
            }
        }
        grid
    }

    /// Buckets per axis.
    pub fn dims(&self) -> [usize; 3] {
        self.dims
    }

    #[inline]
    fn flat(&self, i: usize, j: usize, k: usize) -> usize {
        (k * self.dims[1] + j) * self.dims[0] + i
    }

    /// Bucket index of `p` along each axis, clamped into the grid.
    #[inline]
    fn clamp_index(&self, p: [f64; 3]) -> [usize; 3] {
        let mut idx = [0usize; 3];
        for a in 0..3 {
            let t = ((p[a] - self.lower[a]) / self.width[a]).floor();
            idx[a] = if t.is_nan() || t < 0.0 {
                0
            } else {
                (t as usize).min(self.dims[a] - 1)
            };
        }
        idx
    }

    fn for_each_bucket_in_box<F: FnMut(usize)>(&self, lo: [f64; 3], hi: [f64; 3], mut f: F) {
        let (i0, i1) = (self.clamp_index(lo), self.clamp_index(hi));
        for k in i0[2]..=i1[2] {
            for j in i0[1]..=i1[1] {
                for i in i0[0]..=i1[0] {
                    f(self.flat(i, j, k));
                }
            }
        }
    }

    /// Cells whose bounding box overlaps the bucket containing `p`, or an
    /// empty slice when `p` lies outside the grid (beyond a relative
    /// tolerance of `1e-12` of the bucket width, so a point on the mesh's
    /// outer boundary is still looked up).
    #[inline]
    pub fn candidates_at(&self, p: [f64; 3]) -> &[usize] {
        for a in 0..3 {
            let t = (p[a] - self.lower[a]) / self.width[a];
            let tol = 1e-12 * self.dims[a] as f64;
            if !(t >= -tol && t <= self.dims[a] as f64 + tol) {
                return &[];
            }
        }
        let idx = self.clamp_index(p);
        let b = self.flat(idx[0], idx[1], idx[2]);
        &self.items[self.starts[b]..self.starts[b + 1]]
    }

    /// Every cell whose bucket overlaps the axis-aligned box `[lo, hi]`,
    /// sorted and de-duplicated, appended to `out` (which is cleared first).
    pub fn candidates_in_box(&self, lo: [f64; 3], hi: [f64; 3], out: &mut Vec<usize>) {
        out.clear();
        for a in 0..3 {
            let top = self.lower[a] + self.width[a] * self.dims[a] as f64;
            if hi[a] < self.lower[a] || lo[a] > top {
                return;
            }
        }
        self.for_each_bucket_in_box(lo, hi, |b| {
            out.extend_from_slice(&self.items[self.starts[b]..self.starts[b + 1]]);
        });
        out.sort_unstable();
        out.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two unit cubes side by side: a point in each finds its own cube among
    /// the candidates, and a point outside finds none.
    #[test]
    fn candidates_cover_their_cells() {
        let bounds = [
            [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
            [[1.0, 0.0, 0.0], [2.0, 1.0, 1.0]],
        ];
        let g = CellLocator::build([0.0; 3], [2.0, 1.0, 1.0], &bounds);
        assert!(g.candidates_at([0.5, 0.5, 0.5]).contains(&0));
        assert!(g.candidates_at([1.5, 0.5, 0.5]).contains(&1));
        assert!(g.candidates_at([3.0, 0.5, 0.5]).is_empty());
        let mut out = Vec::new();
        g.candidates_in_box([0.2, 0.2, 0.2], [1.8, 0.4, 0.4], &mut out);
        assert_eq!(out, vec![0, 1]);
    }
}
