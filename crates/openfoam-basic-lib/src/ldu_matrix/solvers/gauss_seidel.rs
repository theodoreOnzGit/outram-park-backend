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

use crate::ldu_matrix::ldu_matrix::LduMatrix;

/// Gauss-Seidel iterative solver for `A·x = b`.
///
/// Performs at most `max_iter` sweeps; stops early when the normalised
/// residual drops below `tol`.  Returns `(iters, final_residual)`.
///
/// Mirrors `Foam::GaussSeidelSmoother` in
/// `src/OpenFOAM/matrices/lduMatrix/smoothers/GaussSeidel/`.
pub fn gauss_seidel(
    mat: &LduMatrix,
    b: &[f64],
    x: &mut Vec<f64>,
    tol: f64,
    max_iter: usize,
) -> (usize, f64) {
    debug_assert_eq!(b.len(), mat.n_cells);
    debug_assert_eq!(x.len(), mat.n_cells);

    // Pre-compute inverse diagonal for efficiency
    let inv_diag: Vec<f64> = mat.diag.iter().map(|d| 1.0 / d).collect();

    let n = mat.n_cells;

    // Build per-cell adjacency once per call: O(n_faces) total.
    // adj[c] = list of (peer, coeff) where the off-diagonal coefficient is coeff.
    // LDU convention: owner[f] < neighbour[f], so processing cells in order
    // gives correct forward Gauss-Seidel (lower-indexed peers already updated).
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for f in 0..mat.n_internal_faces {
        let o  = mat.owner[f];
        let nb = mat.neighbour[f];
        adj[o].push((nb, mat.upper[f]));
        adj[nb].push((o,  mat.lower[f]));
    }

    for iter in 0..max_iter {
        // Forward sweep: O(n_faces) total because Σ adj[c].len() = 2·n_faces.
        for c in 0..n {
            let mut sigma = b[c];
            for &(peer, coeff) in &adj[c] {
                sigma -= coeff * x[peer];
            }
            x[c] = sigma * inv_diag[c];
        }

        let res = mat.normalised_residual(x, b);
        if res < tol {
            return (iter + 1, res);
        }
    }

    let res = mat.normalised_residual(x, b);
    (max_iter, res)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn tridiag_3x3() -> LduMatrix {
        let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
        m.diag  = vec![ 4.0,  4.0,  4.0];  // diagonal dominant
        m.upper = vec![-1.0, -1.0];
        m.lower = vec![-1.0, -1.0];
        m
    }

    #[test]
    fn solves_diagonally_dominant_system() {
        let mat = tridiag_3x3();
        // Known solution: x = [1, 1, 1], b = A·[1,1,1]
        let b = vec![3.0, 2.0, 3.0]; // [4-1, 4-2, 4-1]
        let mut x = vec![0.0; 3];
        let (iters, res) = gauss_seidel(&mat, &b, &mut x, 1e-8, 100);
        assert!(res < 1e-7, "residual {res} after {iters} iters");
        assert_relative_eq!(x[0], 1.0, epsilon = 1e-5);
        assert_relative_eq!(x[1], 1.0, epsilon = 1e-5);
        assert_relative_eq!(x[2], 1.0, epsilon = 1e-5);
    }

    #[test]
    fn diagonal_system_one_iter() {
        // Pure diagonal — Gauss-Seidel solves in 1 iteration
        let mut m = LduMatrix::new(3, vec![], vec![]);
        m.diag = vec![2.0, 3.0, 5.0];
        let b = vec![4.0, 9.0, 15.0];
        let mut x = vec![0.0; 3];
        let (iters, _res) = gauss_seidel(&m, &b, &mut x, 1e-12, 10);
        assert_eq!(iters, 1);
        assert_relative_eq!(x[0], 2.0, epsilon = 1e-12);
        assert_relative_eq!(x[1], 3.0, epsilon = 1e-12);
        assert_relative_eq!(x[2], 3.0, epsilon = 1e-12);
    }
}
