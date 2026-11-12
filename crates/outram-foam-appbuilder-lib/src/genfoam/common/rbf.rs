// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/common/radialBasisFunctionInterpolation/
//                    radialBasisFunctionInterpolation.{C,H}
//                    (the generalised N-dimensional polyharmonic-spline solver
//                     and evaluator used by nuclearDataOneEnergy)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Polyharmonic-spline radial-basis-function interpolation (N-dimensional)
//!
//! This is the numerical kernel behind GeN-Foam's cross-section
//! parametrisation: given a handful of *reference states* (each a point in a
//! multi-parameter feedback space — fuel temperature, coolant density, axial /
//! radial expansion, ...) with a known cross-section value at each, it builds a
//! smooth interpolant that reproduces every state exactly and interpolates in
//! between.
//!
//! ## The interpolant
//!
//! For `N` data points `c_i` in a `p`-dimensional parameter space with values
//! `sigma_i`, the polyharmonic spline is
//!
//! ```text
//!   f(x) = sum_i  w_i phi(||x - c_i||)  +  v_0  +  sum_k v_k x_k
//! ```
//!
//! with radial basis `phi` selected by `mode` (see
//! [`polyharmonic_spline_function`]) and a linear polynomial tail `v`. The
//! weights `(w, v)` solve the symmetric saddle-point system
//!
//! ```text
//!   [ A   B ] [ w ]   [ sigma ]
//!   [ B^T 0 ] [ v ] = [ 0     ]
//! ```
//!
//! where `A_{ij} = phi(||c_i - c_j||)`, and `B` stacks a column of ones with the
//! data coordinates. The orthogonality rows `B^T w = 0` make the polynomial tail
//! well-posed.
//!
//! ## Provenance
//!
//! GeN-Foam keeps these functions in `common/radialBasisFunctionInterpolation`,
//! which the port plan maps to `genfoam::common` — hence this module's home.
//! It ports the two overloads the neutronics cross-section parametrisation and
//! the `multi_region` non-conformal RBF mapping both call (the N-dimensional
//! `solve` + evaluator, plus the shared basis function). It is a faithful
//! port of the upstream algorithm — a saddle-point solve via
//! `outram_foam_basic_lib::matrix::SquareMatrix` (Crout LU with partial
//! pivoting), the direct analogue of upstream's `LUscalarMatrix::inv`.
//!
//! Both consumers —
//! [`crate::genfoam::neutronics::xs::nuclear_data_one_energy`] and
//! [`crate::genfoam::multi_region::rbf_mapping`] — share this single kernel.

use petir::mathf::RealMath;
use outram_foam_basic_lib::matrix::{MatrixError, SquareMatrix};

/// Polyharmonic radial basis function `phi(r)` evaluated from `r^2`.
///
/// `r_square` is the squared Euclidean distance `||x - c||^2` in parameter
/// space; `mode` selects the spline order (GeN-Foam
/// `polyharmonicSplineMode`, default `1`):
///
/// - `1`: `phi = r`            (linear — reproduces linear interpolation in 1-D)
/// - `2`: `phi = r^2 ln(r)`    (thin-plate spline)
/// - `3`: `phi = r^3`
/// - `4`: `phi = r^4 ln(r)`
///
/// Any other `mode` returns `0.0` (matching upstream's out-of-range fallback).
/// Note `phi(0) = 0` in every mode, so the diagonal of `A` is zero.
#[must_use]
pub fn polyharmonic_spline_function(r_square: f64, mode: usize) -> f64 {
    match mode {
        1 => r_square.sqrt(),
        2 => r_square * 0.5 * r_square.r_ln(),
        3 => r_square.sqrt() * r_square,
        4 => r_square * r_square * 0.5 * r_square.r_ln(),
        _ => 0.0,
    }
}

/// Solve for the polyharmonic-spline weights `[w | v]` of an N-dimensional data
/// set, the way upstream does: form `inv(A)` (see
/// [`polyharmonic_spline_inverse`]) and return `inv(A) * [sigma | 0]`, the
/// product accumulated row by row (`SquareMatrix::multiply_vector`).
///
/// Since gh:#1361 (2026-11-12) this is the form of upstream's
/// `solvePolyharmonicSpline`; before it, this function did one direct
/// back-substitution, now [`solve_polyharmonic_spline_direct`]. Against a run
/// of GeN-Foam `652b3da0cc55` the old form differed in 528 of 562 weights
/// (largest `|dw| / max|w|` = 6.4e-14), this one in none; see
/// `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`.
///
/// Arguments, return layout, errors and panics are those of
/// [`solve_polyharmonic_spline_direct`].
pub fn solve_polyharmonic_spline(
    x_list: &[Vec<f64>],
    v_list: &[f64],
    mode: usize,
) -> Result<Vec<f64>, MatrixError> {
    let inverse = polyharmonic_spline_inverse(x_list, mode)?;
    let mut rhs = vec![0.0; inverse.n()];
    rhs[..v_list.len()].copy_from_slice(v_list);
    Ok(inverse.multiply_vector(&rhs))
}

/// Solve for the polyharmonic-spline weights `[w | v]` by one direct
/// LU back-substitution (no inverse). Mathematically the same weights as
/// [`solve_polyharmonic_spline`], different rounding: kept callable as the
/// form this port used before gh:#1361.
///
/// `x_list` is the transposed coordinate table: `x_list[param][data]` is the
/// value of parameter `param` at data point `data`. Every inner list must have
/// the same length `n` (the number of data points). `v_list` holds the `n`
/// data values `sigma_i`. `mode` selects the basis (see
/// [`polyharmonic_spline_function`]).
///
/// Returns the weight vector of length `n + p + 1` (`p = x_list.len()`), laid
/// out as `[w_0..w_{n-1}, v_0, v_1..v_p]` — the `w_i` multiply the radial terms,
/// `v_0` is the constant tail, and `v_1..v_p` the per-parameter linear tail —
/// exactly the layout [`polyharmonic_spline`] expects.
///
/// # Errors
///
/// Returns [`MatrixError::Singular`] if the saddle-point matrix is singular
/// (e.g. duplicate data points). The error is propagated, never defaulted.
///
/// # Panics
///
/// Panics if `x_list` is empty or its rows have unequal length; callers
/// (`nuclearDataOneEnergy::build`) guarantee a rectangular, non-empty table.
pub fn solve_polyharmonic_spline_direct(
    x_list: &[Vec<f64>],
    v_list: &[f64],
    mode: usize,
) -> Result<Vec<f64>, MatrixError> {
    let a = assemble_matrix(x_list, mode);
    let order = a.n();

    // Right-hand side: data values padded with zeros for the orthogonality rows.
    let mut rhs = vec![0.0; order];
    rhs[..v_list.len()].copy_from_slice(v_list);

    a.solve(&rhs)
}

/// The saddle-point matrix `[A B; B^T 0]` of [`solve_polyharmonic_spline`],
/// order `n + p + 1`, assembled as upstream's N-dimensional
/// `solvePolyharmonicSpline` does (`A` symmetric, polynomial-tail rows and
/// columns after the data block).
///
/// # Panics
///
/// Panics if `x_list` is empty or its rows have unequal length.
fn assemble_matrix(x_list: &[Vec<f64>], mode: usize) -> SquareMatrix {
    let n_params = x_list.len();
    assert!(
        n_params >= 1,
        "polyharmonic spline needs at least one parameter"
    );
    let n = x_list[0].len();
    assert!(
        x_list.iter().all(|row| row.len() == n),
        "polyharmonic spline coordinate table must be rectangular"
    );

    let order = n + n_params + 1;
    let mut a = SquareMatrix::new(order);

    for di in 0..n {
        for dj in 0..n {
            if di != dj {
                let mut r_square = 0.0;
                for param in x_list {
                    let d = param[di] - param[dj];
                    r_square += d * d;
                }
                a.set(di, dj, polyharmonic_spline_function(r_square, mode));
            }
            // diagonal stays 0.0 (phi(0) = 0)
        }
        // Polynomial-tail coupling: constant column/row then one per parameter.
        a.set(di, n, 1.0);
        a.set(n, di, 1.0);
        for (param_i, param) in x_list.iter().enumerate() {
            a.set(di, n + param_i + 1, param[di]);
            a.set(n + param_i + 1, di, param[di]);
        }
    }
    a
}

/// The inverse of the saddle-point matrix: the `invRBFmatrix` that upstream
/// `solvePolyharmonicSpline` builds with `LUscalarMatrix::inv` and keeps for
/// later right-hand sides (gh:#1361). Order `n + p + 1`.
///
/// # Errors
///
/// [`MatrixError::Singular`] under the same conditions as
/// [`solve_polyharmonic_spline`].
///
/// # Panics
///
/// Panics if `x_list` is empty or its rows have unequal length.
pub fn polyharmonic_spline_inverse(
    x_list: &[Vec<f64>],
    mode: usize,
) -> Result<SquareMatrix, MatrixError> {
    assemble_matrix(x_list, mode).inverse()
}

/// Evaluate the polyharmonic spline at an arbitrary parameter point `x_input`.
///
/// `weights` is the vector returned by [`solve_polyharmonic_spline`],
/// `x_list[param][data]` the same transposed coordinate table used to build it,
/// and `x_input[param]` the query point (length `p = x_list.len()`). `mode` must
/// match the one used at solve time.
///
/// Returns the interpolated scalar `f(x_input)`.
///
/// # Panics
///
/// Panics if `x_list` is empty; callers guarantee at least one data point.
#[must_use]
pub fn polyharmonic_spline(
    weights: &[f64],
    x_list: &[Vec<f64>],
    x_input: &[f64],
    mode: usize,
) -> f64 {
    let n = x_list[0].len();

    // Polynomial tail: constant + linear-in-each-parameter.
    let mut res = weights[n];
    for (param_i, &xi) in x_input.iter().enumerate() {
        res += weights[n + param_i + 1] * xi;
    }

    // Radial terms.
    for di in 0..n {
        let mut r_square = 0.0;
        for (param_i, &xi) in x_input.iter().enumerate() {
            let d = xi - x_list[param_i][di];
            r_square += d * d;
        }
        if r_square > 0.0 {
            res += weights[di] * polyharmonic_spline_function(r_square, mode);
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> (Vec<Vec<f64>>, Vec<f64>) {
        (
            vec![vec![1.0, 1.0, 2.0, 3.5], vec![2.0, 4.0, 2.0, 3.0]],
            vec![10.0, 20.0, 40.0, 33.0],
        )
    }

    #[test]
    fn both_solves_reproduce_the_data_and_agree_to_round_off() {
        let (x, v) = table();
        for mode in 1..=4 {
            let w_inv = solve_polyharmonic_spline(&x, &v, mode).unwrap();
            let w_dir = solve_polyharmonic_spline_direct(&x, &v, mode).unwrap();
            assert_eq!(w_inv.len(), 4 + 2 + 1);
            for (a, b) in w_inv.iter().zip(&w_dir) {
                assert!((a - b).abs() <= 1e-9 * (1.0 + b.abs()), "mode {mode}: {a} vs {b}");
            }
            for d in 0..4 {
                let q = [x[0][d], x[1][d]];
                let f = polyharmonic_spline(&w_inv, &x, &q, mode);
                assert!((f - v[d]).abs() < 1e-8, "mode {mode} point {d}: {f}");
            }
        }
    }

    #[test]
    fn duplicate_points_are_singular() {
        let x = vec![vec![1.0, 1.0], vec![2.0, 2.0]];
        assert!(polyharmonic_spline_inverse(&x, 1).is_err());
        assert!(solve_polyharmonic_spline(&x, &[1.0, 2.0], 1).is_err());
    }
}
