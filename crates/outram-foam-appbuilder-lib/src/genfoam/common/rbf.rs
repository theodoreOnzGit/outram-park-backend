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
        2 => r_square * 0.5 * r_square.ln(),
        3 => r_square.sqrt() * r_square,
        4 => r_square * r_square * 0.5 * r_square.ln(),
        _ => 0.0,
    }
}

/// Solve for the polyharmonic-spline weights `[w | v]` of an N-dimensional data
/// set.
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
pub fn solve_polyharmonic_spline(
    x_list: &[Vec<f64>],
    v_list: &[f64],
    mode: usize,
) -> Result<Vec<f64>, MatrixError> {
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

    // Right-hand side: data values padded with zeros for the orthogonality rows.
    let mut rhs = vec![0.0; order];
    rhs[..v_list.len()].copy_from_slice(v_list);

    a.solve(&rhs)
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
