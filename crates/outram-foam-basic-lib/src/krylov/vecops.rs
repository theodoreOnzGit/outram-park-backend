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

//! Dense BLAS-1 vector primitives for the Krylov solvers.
//!
//! Pure-Rust replacements for the handful of level-1 BLAS operations the
//! iterative solvers need. All operands are dimensionless `&[f64]` slices whose
//! length equals the number of unknowns (mesh cells); no `uom` typing is applied
//! here because a Krylov subspace mixes residuals, search directions, and
//! solution increments that share no single physical dimension.
//!
//! Every function is `O(n)` in the slice length and allocation-free (results are
//! either scalars or written in place), so they are safe to call inside the
//! innermost solver loops.

/// Euclidean inner product `Σ_i a_i · b_i` (dimensionless).
///
/// Both slices must have the same length; a mismatch panics via
/// `debug_assert`. Valid for any finite inputs; returns `0.0` for empty slices.
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len(), "dot: length mismatch");
    a.iter().zip(b.iter()).map(|(ai, bi)| ai * bi).sum()
}

/// Euclidean 2-norm `sqrt(Σ_i x_i²)` (dimensionless, always `>= 0`).
///
/// Computed as `sqrt(dot(x, x))`. For very large magnitudes this can overflow to
/// `+inf`; inputs are expected to be within normal `f64` range, which holds for
/// well-scaled linear systems.
pub fn nrm2(x: &[f64]) -> f64 {
    dot(x, x).sqrt()
}

/// AXPY update `y := alpha · x + y`, in place.
///
/// `alpha` is a dimensionless scalar; `x` and `y` must have equal length (a
/// mismatch panics via `debug_assert`). `y` is overwritten with the result.
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    debug_assert_eq!(x.len(), y.len(), "axpy: length mismatch");
    for (yi, xi) in y.iter_mut().zip(x.iter()) {
        *yi += alpha * xi;
    }
}

/// Scale `x := alpha · x`, in place.
///
/// `alpha` is a dimensionless scalar. Every element of `x` is multiplied by
/// `alpha`.
pub fn scal(alpha: f64, x: &mut [f64]) {
    for xi in x.iter_mut() {
        *xi *= alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_known() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, -5.0, 6.0];
        // 1*4 + 2*-5 + 3*6 = 4 - 10 + 18 = 12
        assert!((dot(&a, &b) - 12.0).abs() < 1e-12);
    }

    #[test]
    fn nrm2_known() {
        let x = [3.0, 4.0];
        assert!((nrm2(&x) - 5.0).abs() < 1e-12);
        assert!(nrm2(&[]).abs() < 1e-12);
    }

    #[test]
    fn axpy_known() {
        let x = [1.0, 1.0, 1.0];
        let mut y = [10.0, 20.0, 30.0];
        axpy(2.0, &x, &mut y);
        assert_eq!(y, [12.0, 22.0, 32.0]);
    }

    #[test]
    fn scal_known() {
        let mut x = [1.0, -2.0, 3.0];
        scal(-2.0, &mut x);
        assert_eq!(x, [-2.0, 4.0, -6.0]);
    }
}
