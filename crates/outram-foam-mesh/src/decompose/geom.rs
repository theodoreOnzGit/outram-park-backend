// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506, src/parallel/decompose/decompositionMethods:
// geomDecomp/geomDecomp.C, simpleGeomDecomp/simpleGeomDecomp.C,
// hierarchGeomDecomp/hierarchGeomDecomp.C (GPL-3.0-or-later, OpenCFD Ltd).

//! `simple` and `hierarchical` geometric decomposition of cell centres (gh:#1381).
//!
//! Ports of ESI v2506 `simpleGeomDecomp::decomposeOneProc` (unweighted and weighted),
//! `hierarchGeomDecomp::decompose` (serial, unweighted: the `Pstream::nProcs() == 1` branch of
//! `sortComponent`) and `geomDecomp::adjustPoints`, the `delta` rotation that both apply to
//! the points first.
//!
//! **Not ported:** weighted `hierarchical` (its `findBinary` bisection), `manual`, `random`,
//! `multiLevel`, `structured`, and the external-library methods (`scotch`, `metis`, `kahip`).
//! The parallel (`Pstream::parRun`) branches are not ported either: this is the serial path.

use super::std_sort::std_sort_by;

/// Which method of `decomposeParDict` a [`GeomDecomp`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomMethod {
    /// `simpleGeomDecomp`: sort by x, then y, then z, bands of equal size in each.
    Simple,
    /// `hierarchGeomDecomp`: split in the first `order` direction, then each slice in the next.
    Hierarchical,
}

/// The `n`, `order` and `delta` of a `<method>Coeffs` dictionary (`geomDecomp::readCoeffs`).
#[derive(Debug, Clone, PartialEq)]
pub struct GeomDecomp {
    /// Method.
    pub method: GeomMethod,
    /// Divisions in x, y, z (`n`).
    pub n: [usize; 3],
    /// `order` of `hierarchical` as component indices (`xyz` is `[0, 1, 2]`, the default).
    pub order: [usize; 3],
    /// `delta`, default `0.001`: the rotation applied to the points before sorting.
    pub delta: f64,
}

impl GeomDecomp {
    /// A method with `n` divisions and every other entry at its ESI default.
    pub fn new(method: GeomMethod, n: [usize; 3]) -> Self {
        Self { method, n, order: [0, 1, 2], delta: 0.001 }
    }

    /// Set the `order` entry from its dictionary spelling, e.g. `"zyx"`; `None` if it is not
    /// three of `x`, `y`, `z` (`geomDecomp::setOrder` would raise a FatalIOError).
    #[must_use]
    pub fn with_order(mut self, order: &str) -> Option<Self> {
        let chars: Vec<char> = order.chars().collect();
        if chars.len() != 3 {
            return None;
        }
        for (i, c) in chars.iter().enumerate() {
            self.order[i] = match c {
                'x' => 0,
                'y' => 1,
                'z' => 2,
                _ => return None,
            };
        }
        Some(self)
    }

    /// Set `delta`.
    #[must_use]
    pub fn with_delta(mut self, delta: f64) -> Self {
        self.delta = delta;
        self
    }

    /// Number of domains, `n.x * n.y * n.z`.
    pub fn n_domains(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    /// The processor of every point.
    pub fn decompose(&self, points: &[[f64; 3]]) -> Vec<usize> {
        let rotated = self.adjust_points(points);
        match self.method {
            GeomMethod::Simple => simple_decompose(&rotated, self.n),
            GeomMethod::Hierarchical => hierarchical_decompose(&rotated, self.n, self.order, self.n_domains()),
        }
    }

    /// `geomDecomp::adjustPoints`: `csys_.localPosition(points)`. With `delta` equal to 0
    /// (`mag(delta) <= VSMALL`) the system is the identity and the points are returned as they
    /// are; otherwise `R^T & p` with the `specified` rotation `R` ESI builds from `delta`.
    pub fn adjust_points(&self, points: &[[f64; 3]]) -> Vec<[f64; 3]> {
        if (self.delta - 0.0).abs() <= 1e-300 {
            return points.to_vec();
        }
        let r = rotation_from_delta(self.delta);
        // `invTransform(R, v) = R.T() & v`, every component `t.xx*v.x + t.xy*v.y + t.xz*v.z`
        // summed left to right; (R.T)_ij = R_ji.
        points
            .iter()
            .map(|v| {
                let c = |i: usize| (r[0][i] * v[0] + r[1][i] * v[1]) + r[2][i] * v[2];
                [c(0), c(1), c(2)]
            })
            .collect()
    }
}

/// The rotation tensor of `geomDecomp::readCoeffs` for a non-zero `delta`, row major, with each
/// expression evaluated in the order the C++ source writes it.
pub fn rotation_from_delta(delta: f64) -> [[f64; 3]; 3] {
    let d = 1.0 - 0.5 * delta * delta;
    let d2 = d * d;
    let a = delta;
    let a2 = a * a;
    [
        [d2, -a * d, a],
        [a * d - a2 * d, a * a2 + d2, -2.0 * a * d],
        [a * d2 + a2, a * d - a2 * d, d2 - a2],
    ]
}

/// `assignToProcessorGroup(processorGroup, nProcGroup)` of `simpleGeomDecomp.C`: equal bands,
/// the first `size - jump*n` bands one longer.
fn assign_equal(group: &mut [usize], n_group: usize) {
    let jump = group.len() / n_group;
    let first_longer = group.len() - jump * n_group;
    let mut ind = 0;
    let mut j = 0;
    while j < first_longer {
        for _ in 0..=jump {
            group[ind] = j;
            ind += 1;
        }
        j += 1;
    }
    while j < n_group {
        for _ in 0..jump {
            group[ind] = j;
            ind += 1;
        }
        j += 1;
    }
}

/// Weighted `assignToProcessorGroup` of `simpleGeomDecomp.C`.
fn assign_weighted(group: &mut [usize], n_group: usize, indices: &[usize], weights: &[f64], summed: f64) {
    let jump = summed / n_group as f64;
    let mut sum_weights = 0.0;
    let mut ind = 0;
    for j in 0..n_group - 1 {
        let limit = jump * (j + 1) as f64;
        while sum_weights < limit {
            sum_weights += weights[indices[ind]];
            group[ind] = j;
            ind += 1;
        }
    }
    while ind < group.len() {
        group[ind] = n_group - 1;
        ind += 1;
    }
}

/// `simpleGeomDecomp::decomposeOneProc(points)`: `points` are already rotated.
pub fn simple_decompose(points: &[[f64; 3]], n: [usize; 3]) -> Vec<usize> {
    simple_decompose_weighted(points, n, &[])
}

/// `simpleGeomDecomp::decomposeOneProc(points, weights)`; empty `weights` is the unweighted path.
pub fn simple_decompose_weighted(points: &[[f64; 3]], n: [usize; 3], weights: &[f64]) -> Vec<usize> {
    let np = points.len();
    let mut result = vec![0usize; np];
    let mut groups = vec![0usize; np];
    let mut idx: Vec<usize> = (0..np).collect();
    let summed: f64 = weights.iter().sum();
    let mult = [1, n[0], n[0] * n[1]];
    for dir in 0..3 {
        std_sort_by(&mut idx, |a, b| points[a][dir] < points[b][dir]);
        if weights.is_empty() {
            assign_equal(&mut groups, n[dir]);
        } else {
            assign_weighted(&mut groups, n[dir], &idx, weights, summed);
        }
        for i in 0..np {
            result[idx[i]] += mult[dir] * groups[i];
        }
    }
    result
}

/// `hierarchGeomDecomp::decompose` on rotated `points`, serial and unweighted.
pub fn hierarchical_decompose(points: &[[f64; 3]], n: [usize; 3], order: [usize; 3], n_domains: usize) -> Vec<usize> {
    let _ = n_domains; // `sizeTol` is only used by the parallel bisection
    let mut result = vec![0usize; points.len()];
    let slice: Vec<usize> = (0..points.len()).collect();
    sort_component(points, &slice, 0, 1, n, order, &mut result);
    result
}

/// `hierarchGeomDecomp::sortComponent` (unweighted, `Pstream::nProcs() == 1`).
fn sort_component(
    points: &[[f64; 3]],
    current: &[usize],
    component_index: usize,
    mult: usize,
    n: [usize; 3],
    order: [usize; 3],
    result: &mut [usize],
) {
    let comp = order[component_index];
    // SortableList::sort is std::stable_sort of the indices by value (less).
    let mut sorted: Vec<usize> = (0..current.len()).collect();
    let val = |i: usize| points[current[i]][comp];
    sorted.sort_by(|&a, &b| {
        if val(a) < val(b) {
            std::cmp::Ordering::Less
        } else if val(b) < val(a) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    let mut left = 0;
    for bin in 0..n[comp] {
        let local = if bin == n[comp] - 1 { current.len() - left } else { current.len() / n[comp] };
        let slice: Vec<usize> = (0..local).map(|i| current[sorted[left + i]]).collect();
        for &p in &slice {
            result[p] += bin * mult;
        }
        if component_index < 2 {
            sort_component(points, &slice, component_index + 1, mult * n[comp], n, order, result);
        }
        left += local;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(nx: usize, ny: usize, nz: usize) -> Vec<[f64; 3]> {
        let mut p = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    p.push([i as f64 + 0.5, j as f64 + 0.5, k as f64 + 0.5]);
                }
            }
        }
        p
    }

    #[test]
    fn equal_bands_put_the_extra_cells_first() {
        let mut g = vec![0; 7];
        assign_equal(&mut g, 3);
        assert_eq!(g, vec![0, 0, 0, 1, 1, 2, 2]);
    }

    #[test]
    fn weighted_bands_follow_the_cumulative_weight() {
        let mut g = vec![0; 4];
        assign_weighted(&mut g, 2, &[0, 1, 2, 3], &[1.0, 1.0, 3.0, 3.0], 8.0);
        assert_eq!(g, vec![0, 0, 0, 1]);
        // the cell that crosses the limit is still in the lower group (the while adds before it tests)
        assign_weighted(&mut g, 2, &[0, 1, 2, 3], &[1.0, 1.0, 1.0, 5.0], 8.0);
        assert_eq!(g, vec![0, 0, 0, 0]);
        assign_weighted(&mut g, 2, &[0, 1, 2, 3], &[1.0, 1.0, 3.0, 3.0], 8.0);
        assert_eq!(g, vec![0, 0, 0, 1]);
    }

    /// A grid with `delta 0` splits into the obvious bricks, whatever the tie order.
    #[test]
    fn simple_on_a_grid_makes_equal_counts() {
        let p = grid(4, 4, 1);
        let g = GeomDecomp::new(GeomMethod::Simple, [2, 2, 1]);
        let d = g.decompose(&p);
        let mut count = [0; 4];
        for &x in &d {
            count[x] += 1;
        }
        assert_eq!(count, [4, 4, 4, 4]);
        let w = simple_decompose_weighted(&p, [2, 2, 1], &vec![1.0; p.len()]);
        let mut count = [0; 4];
        for &x in &w {
            count[x] += 1;
        }
        assert_eq!(count, [4, 4, 4, 4]);
    }

    #[test]
    fn hierarchical_gives_bricks_in_the_requested_order() {
        let p = grid(4, 2, 1);
        let g = GeomDecomp::new(GeomMethod::Hierarchical, [2, 2, 1]).with_delta(0.0);
        let d = g.decompose(&p);
        // x bins are 0 (x<2) and 1; y bins add 2: bricks
        assert_eq!(d, vec![0, 0, 1, 1, 2, 2, 3, 3]);
        let g = GeomDecomp::new(GeomMethod::Hierarchical, [2, 2, 1]).with_order("yxz").unwrap().with_delta(0.0);
        // first split is y (mult 1), then x (mult 2)
        assert_eq!(g.decompose(&p), vec![0, 0, 2, 2, 1, 1, 3, 3]);
    }

    #[test]
    fn order_parsing_and_rotation() {
        assert!(GeomDecomp::new(GeomMethod::Simple, [1, 1, 1]).with_order("xy").is_none());
        assert!(GeomDecomp::new(GeomMethod::Simple, [1, 1, 1]).with_order("xyq").is_none());
        let r = rotation_from_delta(0.001);
        // a rotation to first order: diagonal near 1, rows nearly orthogonal
        assert!((r[0][0] - 1.0).abs() < 1e-5 && (r[1][1] - 1.0).abs() < 1e-5);
        let g = GeomDecomp::new(GeomMethod::Simple, [2, 1, 1]);
        let q = g.adjust_points(&[[1.0, 2.0, 3.0]]);
        assert!((q[0][0] - 1.0).abs() < 1e-2 && (q[0][2] - 3.0).abs() < 1e-2);
        assert_eq!(g.with_delta(0.0).adjust_points(&[[1.0, 2.0, 3.0]]), vec![[1.0, 2.0, 3.0]]);
    }
}
